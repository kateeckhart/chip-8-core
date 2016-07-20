extern crate sdl2;
extern crate time;
use std::fs::File;
use std::io::prelude::*;
use sdl2::event::*;
use time::PreciseTime;
use time::Duration;

struct Chip8 {
    data_registers: [u8; 16],
    address_register: u16,
    memory: [u8; 0x1000],
    program_counter: u16,
    stack: Vec<u16>,
    delay_timer: u8,
    sound_timer: u8,
    frame_buffer: [[u8; 8]; 4], // Bit set white, bit unset black
}

fn convert_address(nibble: u8, byte: u8) -> u16 {
    let mut address = nibble as u16;
    address <<= 8;
    address | byte as u16
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
        delay_timer: 0,
        sound_timer: 0,
        frame_buffer: [[0; 8]; 4]
    };
    if let Some(file) = args.next() {
        match File::open(file) {
            Ok(mut input_file) => {
                let len = chip8.memory.len();
                let mut program_mem = &mut chip8.memory[0x200..len];
                input_file.read(program_mem).unwrap();
            }
            Err(error) => {
                println!("{}", error);
                return;
            }
        }
    } else {
        println!("Please provide the program you want to run");
        return;
    }
    let sdl = sdl2::init().unwrap();
    let sdl_video = sdl.video().unwrap();
    let sdl_window = sdl_video.window("Chip-8", 64 * 8, 32 * 8)
        .resizable()
        .build()
        .unwrap();
    let mut sdl_event_pump = sdl.event_pump().unwrap();
    let mut sdl_renderer = sdl_window.renderer().present_vsync().build().unwrap();
    sdl_renderer.set_logical_size(64, 32).unwrap();
    sdl_renderer.present();
    let mut v_blank_begin = PreciseTime::now();
    let next_v_blank = Duration::microseconds(16667);
    let mut running = true;
    while running {
        if v_blank_begin.to(PreciseTime::now()) < next_v_blank {
            let optcode_byte_1 = chip8.memory[chip8.program_counter as usize];
            let optcode_nibble_1 = optcode_byte_1 >> 4;
            let optcode_nibble_2 = optcode_byte_1 & 0x0f;
            let optcode_byte_2 = chip8.memory[chip8.program_counter as usize + 1];
            let optcode_nibble_3 = optcode_byte_2 >> 4;
            let optcode_nibble_4 = optcode_byte_2 & 0x0f;
            match optcode_nibble_1 {
                0 => {
                    if optcode_nibble_2 != 0x00 {
                    println!("Unimplemented optcode");
                    running = false;
                    }
                    match optcode_byte_2 {
                        0xE0 => chip8.frame_buffer = [[0; 8]; 4],
                        0xEE => { 
                            if let Some(x) = chip8.stack.pop() {
                                chip8.program_counter = x;
                                continue;
                            } else {
                                println!("Stack underflow");
                                running = false;
                            }
                        }
                        0xFF => println!("Good"), // Debug optcode
                        _ => {
                            println!("Unimplemented optcode");
                            running = false;
                        }
                    }
                }
                1 => {
                    chip8.program_counter = convert_address(optcode_nibble_2, optcode_byte_2);
                    continue;
                }
                2 => {
                    chip8.stack.push(chip8.program_counter + 2);
                    chip8.program_counter = convert_address(optcode_nibble_2, optcode_byte_2);
                    continue;
                }
                6 => chip8.data_registers[optcode_nibble_2 as usize] = optcode_byte_2,
                _ => {
                    println!("Unimplemented optcode");
                    running = false;
                }
            }
            chip8.program_counter += 2;
        } else {
            match sdl_event_pump.wait_event() {
                Event::Quit { timestamp: _ } => running = false,
                _ => {},
            }
            sdl_renderer.clear();
            sdl_renderer.present();
            if chip8.delay_timer > 0 {
                chip8.delay_timer -= 1;
            }
            if chip8.sound_timer > 0 {
                chip8.sound_timer -= 1;
            }
            v_blank_begin = PreciseTime::now();
        }
    }
    println!("{0:x}", chip8.program_counter);
}
