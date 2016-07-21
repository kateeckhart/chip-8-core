extern crate sdl2;
extern crate time;
extern crate rand;
use std::fs::File;
use std::io::prelude::*;
use sdl2::event::*;
use sdl2::rect::*;
use time::PreciseTime;
use time::Duration;
use rand::Rng;

struct Chip8 {
    data_registers: [u8; 16],
    address_register: u16,
    memory: [u8; 0x1000],
    program_counter: u16,
    stack: Vec<u16>,
    delay_timer: u8,
    sound_timer: u8,
    frame_buffer: [[u8; 8]; 32], // Bit set white, bit unset black
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
        frame_buffer: [[0; 8]; 32],
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
    let next_v_blank = Duration::microseconds(16664);
    let mut running = true;
    let mut rng = rand::thread_rng();
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
                        0xE0 => chip8.frame_buffer = [[0; 8]; 32],
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
                3 => {
                    if chip8.data_registers[optcode_nibble_2 as usize] == optcode_byte_2 {
                        chip8.program_counter += 2;
                    }
                }
                4 => {
                    if chip8.data_registers[optcode_nibble_2 as usize] != optcode_byte_2 {
                        chip8.program_counter += 2;
                    }
                }
                5 => {
                    if chip8.data_registers[optcode_nibble_4 as usize] != 0 {
                        println!("Unimplemented optcode");
                        running = false;
                    }
                    if chip8.data_registers[optcode_nibble_2 as usize] ==
                       chip8.data_registers[optcode_nibble_3 as usize] {
                        chip8.program_counter += 2;
                    }
                }
                6 => chip8.data_registers[optcode_nibble_2 as usize] = optcode_byte_2,
                7 => chip8.data_registers[optcode_nibble_2 as usize] += optcode_byte_2,
                8 => {
                    match optcode_nibble_4 {
                        0 => {
                            chip8.data_registers[optcode_nibble_2 as usize] =
                                chip8.data_registers[optcode_nibble_3 as usize]
                        }
                        1 => {
                            chip8.data_registers[optcode_nibble_2 as usize] |=
                                chip8.data_registers[optcode_nibble_3 as usize]
                        }
                        2 => {
                            chip8.data_registers[optcode_nibble_2 as usize] &=
                                chip8.data_registers[optcode_nibble_3 as usize]
                        }
                        3 => {
                            chip8.data_registers[optcode_nibble_2 as usize] ^=
                                chip8.data_registers[optcode_nibble_3 as usize]
                        }
                        4 => {
                            let (added, overflow) = chip8.data_registers[optcode_nibble_2 as usize]
                                .overflowing_add(chip8.data_registers[optcode_nibble_3 as usize]);
                            chip8.data_registers[optcode_nibble_2 as usize] = added;
                            chip8.data_registers[0xF] = overflow as u8;
                        }
                        5 => {
                            let (subed, overflow) = chip8.data_registers[optcode_nibble_2 as usize]
                                .overflowing_sub(chip8.data_registers[optcode_nibble_3 as usize]);
                            chip8.data_registers[optcode_nibble_2 as usize] = subed;
                            chip8.data_registers[0xF] = overflow as u8;
                        }
                        6 => {
                            let lsb = chip8.data_registers[optcode_nibble_2 as usize] & 1;
                            chip8.data_registers[optcode_nibble_2 as usize] -= lsb;
                            chip8.data_registers[optcode_nibble_2 as usize] >>= 1;
                            chip8.data_registers[0xF] = lsb;
                        }
                        7 => {
                            let (subed, overflow) = chip8.data_registers[optcode_nibble_3 as usize]
                                .overflowing_sub(chip8.data_registers[optcode_nibble_2 as usize]);
                            chip8.data_registers[optcode_nibble_2 as usize] = subed;
                            chip8.data_registers[0xF] = overflow as u8;
                        }
                        0xE => {
                            let msb = chip8.data_registers[optcode_nibble_2 as usize] & 0x80;
                            chip8.data_registers[optcode_nibble_2 as usize] -= msb;
                            chip8.data_registers[optcode_nibble_2 as usize] <<= 1;
                            chip8.data_registers[0xF] = msb;
                        }
                        _ => {
                            println!("Unimplemented optcode");
                            running = false;
                        }
                    }
                }
                9 => {
                    if chip8.data_registers[optcode_nibble_4 as usize] != 0 {
                        println!("Unimplemented optcode");
                        running = false;
                    }
                    if chip8.data_registers[optcode_nibble_2 as usize] !=
                       chip8.data_registers[optcode_nibble_3 as usize] {
                        chip8.program_counter += 2;
                    }
                }
                0xA => chip8.address_register = convert_address(optcode_nibble_2, optcode_byte_2),
                0xB => {
                    chip8.program_counter = convert_address(optcode_nibble_2, optcode_byte_2);
                    chip8.program_counter += chip8.data_registers[0] as u16;
                    continue;
                }
                0xC => {
                    let rand: u8 = rng.gen();
                    chip8.data_registers[optcode_nibble_2 as usize] = rand & optcode_byte_2;
                }
                0xD => {
                    chip8.data_registers[0xf] = 0;
                    for i in chip8.memory[chip8.address_register as usize..chip8.address_register as usize + optcode_nibble_4 as usize]
                             .iter()
                             .enumerate() {
                        let (mut y_position, y) = i;
                        y_position += chip8.data_registers[optcode_nibble_3 as usize] as usize;
                        for x_position_ram in 0..8 {
                            let mut x_position_buff = x_position_ram + chip8.data_registers[optcode_nibble_2 as usize];
                            let mut x_position_buff_bytes = x_position_buff % 8;
                            let mut x_position_buff_bits = x_position_buff / 8;
                            x_position_buff_bits = 7 - x_position_buff_bits;
                            x_position_buff_bytes *= 8;
                            x_position_buff = x_position_buff_bits + x_position_buff_bytes;
                            let x_byte = x_position_buff / 8;
                            let mut x_bit_ram = x_position_ram % 8;
                            let mut x_bit_buff = x_position_buff % 8;
                            x_bit_ram = 1 << x_bit_ram;
                            x_bit_buff = 7 - x_bit_buff;
                            x_bit_buff = 1 << x_bit_buff;
                            if y & x_bit_ram != 0 {
                                if chip8.frame_buffer[y_position][x_byte as usize] & x_bit_buff != 0 {
                                    chip8.frame_buffer[y_position][x_byte as usize] ^= x_bit_buff;
                                    chip8.data_registers[0xf] = 1;
                                } else {
                                    chip8.frame_buffer[y_position][x_byte as usize] |= x_bit_buff;
                                }
                            }
                        }
                    }
                }
                _ => {
                    println!("Unimplemented optcode");
                    running = false;
                }
            }
            chip8.program_counter += 2;
        } else {
            match sdl_event_pump.wait_event() {
                Event::Quit { timestamp: _ } => running = false,
                _ => {}
            }
            sdl_renderer.clear();
            for i in chip8.frame_buffer.iter().enumerate() {
                let (y_cord, y) = i;
                for i in y.iter().enumerate() {
                    let (x_cord, x) = i;
                    let x_cord = x_cord * 8;
                    for b in 0..8 {
                        let x_cord = x_cord + b;
                        let b = 1 << b;
                        if x & b != 0 {
                            sdl_renderer.draw_point(Point::new(x_cord as i32, y_cord as i32))
                                .unwrap();
                        }
                    }
                }
            }
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
}
