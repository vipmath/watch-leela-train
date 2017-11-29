#![feature(inclusive_range_syntax)]
extern crate regex;

use std::process::{Command,Stdio};
use std::io::{BufRead, BufReader};
use std::str;
use std::ops::{Index, IndexMut};
use std::env;
use std::collections::HashSet;

use regex::Regex;

const SIZE: usize = 19;
const ROW_INDICES: &'static str = "ABCDEFGHJKLMNOPQRST";

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
struct Coord(usize, usize);

#[derive(Debug, Clone, Copy)]
enum Player {
    White,
    Black,
}

impl Player {
    fn symbol(&self) -> char {
        match *self {
            Player::White => 'o',
            Player::Black => 'x',
        }
    }
}

struct Board {
    board: Vec<char>,
    size: usize
}

impl Index<Coord> for Board {
    type Output = char;

    fn index(&self, coord: Coord) -> &char {
        &self.board[get_index(self.size, coord)]
    }
}

impl IndexMut<Coord> for Board {
    fn index_mut(&mut self, coord: Coord) -> &mut char {
        &mut self.board[get_index(self.size, coord)]
    }
}

impl Board {
    fn new(size: usize) -> Board {
        Board {
            board: vec!['.'; size * size],
            size: size,
        }
    }

    fn to_string(&self) -> String {
        let mut out = String::new();
        for row in 1..=self.size {
            out.push('\n');
            for col in 1..=self.size {
                out.push(self[Coord(row, col)]);
                out.push(' ');
            }
        }
        return out;
    }
}

fn get_index(size: usize, Coord(row, col): Coord) -> usize {
    return (row - 1) * size + (col - 1)
}

fn update_board(board: &mut Board, row_str: &str, col_str: &str, current_player: Player) -> Vec<u8> {
    let row_index = ROW_INDICES.find(row_str).unwrap() + 1;
    let col_index: usize = col_str.parse().unwrap();
    board[Coord(row_index, col_index)] = current_player.symbol();

    resolve_capture(board)
}

fn resolve_capture(board: &mut Board) -> Vec<u8>{
    let mut out = Vec::new();
    let mut processed_coords: HashSet<Coord> = HashSet::new();
    let mut chain_coords: HashSet<Coord> = HashSet::new();
    let mut liberties: HashSet<Coord> = HashSet::new();
    for row in 1..=board.size {
        for col in 1..=board.size {
            let coord = Coord(row, col);
            if board[coord] != '.' && ! processed_coords.contains(&coord) {
                process_chain(board, coord, &mut chain_coords, &mut liberties);
                if liberties.len() == 0 {
                    out.push(chain_coords.len() as u8);
                    for &chain_coord in &chain_coords {
                        board[chain_coord] = '.';
                    }
                }
                processed_coords.extend(chain_coords.drain()); // this clears chain_coords
                liberties.clear()
            }
        }
    }
    out
}

fn process_chain(board: &Board, coord: Coord, chain_coords: &mut HashSet<Coord>, liberties: &mut HashSet<Coord>) {
    let Coord(row, col) = coord;
    if row < 1 || col < 1 || row > board.size || col > board.size {
        return;
    }
    
    let val = board[coord];
    if val == '.' {
        liberties.insert(coord);
        return;
    }
    else if ! chain_coords.contains(&coord) 
            && (chain_coords.len() == 0
                || val == board[*chain_coords.iter().next().unwrap()]
                ){
        chain_coords.insert(coord);
        process_chain(board, Coord(row - 1, col), chain_coords, liberties);
        process_chain(board, Coord(row + 1, col), chain_coords, liberties);
        process_chain(board, Coord(row, col - 1), chain_coords, liberties);
        process_chain(board, Coord(row, col + 1), chain_coords, liberties);
    }
    
}

fn get_autogtp_version() -> String {
    let output = Command::new("./autogtp")
        .arg("--version")
        .output()
        .expect("failed to start autogtp");
    String::from_utf8_lossy(&output.stdout).trim_right().split(' ').last()
        .expect("Failed to determine autogtp version!").into()
}

fn main() {
    let autogtp_version = get_autogtp_version();

    let mut arguments: Vec<_> = env::args().skip(1).collect();
    if arguments.len() < 2 {
        arguments = vec!["-k".to_string(), "sgfs".to_string()];
    }
    let mut child = Command::new("./autogtp")
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start autogtp");

    let mut child_out = BufReader::new(child.stdout.as_mut().unwrap());
    let mut child_err = BufReader::new(child.stderr.as_mut().unwrap());
    
    let mut buffer: Vec<u8> = Vec::new();
    let mut line: String;

    let mut board: Board = Board::new(SIZE);
    let mut current_player = Player::Black;
    let move_regex = Regex::new(r"[ \n]\d+ \(([A-Z])(\d+)\)").unwrap();
    let end_regex = Regex::new(r"Game has ended").unwrap();
    let move_or_pass_regex = Regex::new(r"\((?:[A-Z]\d+)|(?:pass)\)").unwrap();
    assert!(move_regex.is_match(" 245 (F18)"));

    loop {
        // Early autogtp versions printed to stderr instead of stdout
        if vec!["v1", "v2", "v3", "v4"].contains(&&autogtp_version[..]) { 
            child_err.read_until(')' as u8, &mut buffer).unwrap();
        } else {
            child_out.read_until(')' as u8, &mut buffer).unwrap();
        }

        line = String::from_utf8_lossy(&buffer).into();
        let mut out = line.clone();
        buffer.clear();

        if end_regex.is_match(&line) {
            board = Board::new(SIZE);
            current_player = Player::Black;
        }

        match move_regex.captures(&line) {
            Some(caps) => {
                let captures = update_board(
                    &mut board,
                    caps.get(1).unwrap().as_str(),
                    caps.get(2).unwrap().as_str(),
                    current_player);
                if !captures.is_empty() {
                    out.push_str(&format!(" Captured chains: {}",
                        captures.iter().map(|c| c.to_string())
                            .collect::<Vec<String>>().join(", ")
                        ))
                }
            },
            _ => {},
        }
        
        if move_or_pass_regex.is_match(&line) {
            out.push_str(&board.to_string());

            current_player = match current_player {
                Player::White => Player::Black,
                Player::Black => Player::White,
            }
        }
        out.push('\n');
        print!("{}", out);
    }
}
