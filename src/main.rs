use std::fs::File;
use std::io::prelude::*;

struct Chip8 {
    data_registers: [u8; 16],
    address_register: u16,
    memory: [u8; 0x1000],
    program_counter: u16,
    stack: Vec<u16>,
}

fn main() {
    let mut args = std::env::args();
    args.next(); // We do not need the path of the executable.
    let mut chip8 = Chip8 {
        data_registers: [0; 16],
        address_register: 0,
        memory: [0; 0x1000],
        program_counter: 0x200, // Entry point of most programs
        stack: Vec::with_capacity(16),
    };
    if let Some(file) = args.next() {
        match File::open(file) {
            Ok(mut input_file) => {
                let len = chip8.memory.len();
                let mut program_mem = &mut chip8.memory[0x200..len];
                input_file.read(program_mem); },
            Err(error) => println!("{}", error),
        }
    } else {
        println!("Please provide the program you want to run");
        return;
    }
}
