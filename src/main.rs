extern crate sdl2;
extern crate time;
use std::fs::File;
use std::io::prelude::*;
use sdl2::event::*;
use sdl2::pixels::*;
use sdl2::render::*;
use sdl2::rect::*;
use sdl2::surface::*;
use time::PreciseTime;
use time::Duration;

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
        } else {
            match sdl_event_pump.wait_event() {
                Event::Quit { timestamp: _ } => running = false,
                _ => {},
            }
            sdl_renderer.clear();
            sdl_renderer.present();
            v_blank_begin = PreciseTime::now();
        }
    }
}
