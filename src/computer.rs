extern crate rand;
use rand::Rng;
use std::fs::File;
use std::io::prelude::*;
use std::io::Error;

pub trait KeyWrapper {
    fn is_pushed(&self, u8) -> Result<bool, &'static str>;
    fn get_key(&self) -> Option<u8>;
}

pub trait AudioWrapper {
    fn play(&mut self);
    fn stop(&mut self);
}

pub struct Chip8<T: KeyWrapper> {
    data_registers: [u8; 16],
    address_register: u16,
    memory: [u8; 0x1000],
    program_counter: u16,
    stack: Vec<u16>,
    delay_timer: u8,
    pub sound_timer: u8,
    pub frame_buffer: [[u8; 64]; 32], // Byte == zero black, byte == one white
    rng: rand::ThreadRng,
    pub key_wrapper: T,
}

fn convert_address(nibble: u8, byte: u8) -> u16 {
    let mut address = nibble as u16;
    address <<= 8;
    address | byte as u16
}

impl<T: KeyWrapper> Chip8<T> {
    pub fn new(key_wrapper: T) -> Chip8<T> {
        let mut chip8 = Chip8 {
            data_registers: [0; 16],
            address_register: 0,
            memory: [0; 0x1000],
            program_counter: 0x200, // Entry point of most programs
            stack: Vec::with_capacity(16),
            delay_timer: 0,
            sound_timer: 0,
            frame_buffer: [[0; 64]; 32],
            rng: rand::thread_rng(),
            key_wrapper: key_wrapper,
        };
        let font = include_bytes!("font.bin");
        chip8.memory[0..font.len()].copy_from_slice(font);
        chip8
    }
    pub fn load_prog_from_file(&mut self, input_file: &mut File) -> Result<usize, Error> {
        let len = self.memory.len();
        let mut program_mem = &mut self.memory[0x200..len];
        input_file.read(program_mem)
    }
    pub fn run_optcode(&mut self) -> Result<(), &'static str> {
        let optcode_byte_1 = self.memory[self.program_counter as usize];
        let optcode_nibble_1 = optcode_byte_1 >> 4;
        let optcode_nibble_2 = optcode_byte_1 & 0x0f;
        let optcode_byte_2 = self.memory[self.program_counter as usize + 1];
        let optcode_nibble_3 = optcode_byte_2 >> 4;
        let optcode_nibble_4 = optcode_byte_2 & 0x0f;
        match optcode_nibble_1 {
            0 => {
                if optcode_nibble_2 != 0x00 {
                    return Err("Unimplemented optcode");
                }
                match optcode_byte_2 {
                    0xE0 => self.frame_buffer = [[0; 64]; 32],
                    0xEE => {
                        if let Some(x) = self.stack.pop() {
                            self.program_counter = x;
                            return Ok(());
                        } else {
                            return Err("Stack underflow");
                        }
                    }
                    _ => {
                        return Err("Unimplemented optcode");
                    }
                }
            }
            1 => {
                self.program_counter = convert_address(optcode_nibble_2, optcode_byte_2);
                return Ok(());
            }
            2 => {
                self.stack.push(self.program_counter + 2);
                self.program_counter = convert_address(optcode_nibble_2, optcode_byte_2);
                return Ok(());
            }
            3 => {
                if self.data_registers[optcode_nibble_2 as usize] == optcode_byte_2 {
                    self.program_counter += 2;
                }
            }
            4 => {
                if self.data_registers[optcode_nibble_2 as usize] != optcode_byte_2 {
                    self.program_counter += 2;
                }
            }
            5 => {
                if self.data_registers[optcode_nibble_4 as usize] != 0 {
                    return Err("Unimplemented optcode");
                }
                if self.data_registers[optcode_nibble_2 as usize] ==
                   self.data_registers[optcode_nibble_3 as usize] {
                    self.program_counter += 2;
                }
            }
            6 => self.data_registers[optcode_nibble_2 as usize] = optcode_byte_2,
            7 => {
                self.data_registers[optcode_nibble_2 as usize] =
                    self.data_registers[optcode_nibble_2 as usize].wrapping_add(optcode_byte_2)
            }
            8 => {
                match optcode_nibble_4 {
                    0 => {
                        self.data_registers[optcode_nibble_2 as usize] =
                            self.data_registers[optcode_nibble_3 as usize]
                    }
                    1 => {
                        self.data_registers[optcode_nibble_2 as usize] |=
                            self.data_registers[optcode_nibble_3 as usize]
                    }
                    2 => {
                        self.data_registers[optcode_nibble_2 as usize] &=
                            self.data_registers[optcode_nibble_3 as usize]
                    }
                    3 => {
                        self.data_registers[optcode_nibble_2 as usize] ^=
                            self.data_registers[optcode_nibble_3 as usize]
                    }
                    4 => {
                        let (added, overflow) = self.data_registers[optcode_nibble_2 as usize]
                            .overflowing_add(self.data_registers[optcode_nibble_3 as usize]);
                        self.data_registers[optcode_nibble_2 as usize] = added;
                        self.data_registers[0xF] = overflow as u8;
                    }
                    5 => {
                        let (subed, mut overflow) = self.data_registers[optcode_nibble_2 as usize]
                            .overflowing_sub(self.data_registers[optcode_nibble_3 as usize]);
                        self.data_registers[optcode_nibble_2 as usize] = subed;
                        overflow = !overflow; // Inverted borrow_flag
                        self.data_registers[0xF] = overflow as u8;
                    }
                    6 => {
                        let lsb = self.data_registers[optcode_nibble_2 as usize] & 1;
                        self.data_registers[optcode_nibble_2 as usize] -= lsb;
                        self.data_registers[optcode_nibble_2 as usize] >>= 1;
                        self.data_registers[0xF] = lsb;
                    }
                    7 => {
                        let (subed, mut overflow) = self.data_registers[optcode_nibble_3 as usize]
                            .overflowing_sub(self.data_registers[optcode_nibble_2 as usize]);
                        self.data_registers[optcode_nibble_2 as usize] = subed;
                        overflow = !overflow; // Inverted borrow_flag
                        self.data_registers[0xF] = overflow as u8;
                    }
                    0xE => {
                        let mut msb = self.data_registers[optcode_nibble_2 as usize] & 0x80;
                        self.data_registers[optcode_nibble_2 as usize] -= msb;
                        self.data_registers[optcode_nibble_2 as usize] <<= 1;
                        msb >>= 7;
                        self.data_registers[0xF] = msb;
                    }
                    _ => {
                        return Err("Unimplemented optcode");
                    }
                }
            }
            9 => {
                if optcode_nibble_4 != 0 {
                    return Err("Unimplemented optcode");
                }
                if self.data_registers[optcode_nibble_2 as usize] !=
                   self.data_registers[optcode_nibble_3 as usize] {
                    self.program_counter += 2;
                }
            }
            0xA => self.address_register = convert_address(optcode_nibble_2, optcode_byte_2),
            0xB => {
                self.program_counter = convert_address(optcode_nibble_2, optcode_byte_2);
                self.program_counter += self.data_registers[0] as u16;
                return Ok(());
            }
            0xC => {
                let rand: u8 = self.rng.gen();
                self.data_registers[optcode_nibble_2 as usize] = rand & optcode_byte_2;
            }
            0xD => {
                self.data_registers[0xF] = 0;
                for i in
                    self.memory[self.address_register as usize..self.address_register as usize +
                                                                optcode_nibble_4 as usize]
                    .iter()
                    .enumerate() {
                    let (mut y_position, y) = i;
                    y_position += self.data_registers[optcode_nibble_3 as usize] as usize;
                    if y_position > 0x1F {
                        continue;
                    }
                    for b in 0..8 {
                        let b_shifted = 1 << b;
                        let mut bit = y & b_shifted;
                        bit >>= b;
                        if bit == 1 {
                            let inverted = 7 - b;
                            let mut x_position =
                                self.data_registers[optcode_nibble_2 as usize] as usize +
                                inverted as usize;
                            x_position %= 0x40; // The screen wraps around
                            if self.frame_buffer[y_position][x_position] ^ bit == 0 {
                                self.frame_buffer[y_position][x_position] = 0;
                                self.data_registers[0xF] = 1;
                            } else {
                                self.frame_buffer[y_position][x_position] = 1;
                            }
                        }
                    }
                }
            }
            0xE => {
                match optcode_byte_2 {
                    0x9E => {
                        if try!(self.key_wrapper
                            .is_pushed(self.data_registers[optcode_nibble_2 as usize])) {
                            self.program_counter += 2;
                        }
                    }
                    0xA1 => {
                        if !try!(self.key_wrapper
                            .is_pushed(self.data_registers[optcode_nibble_2 as usize])){
                            self.program_counter += 2;
                        }
                    }
                    _ => {
                        return Err("Unimplemented optcode");
                    }
                }
            }
            0xF => {
                match optcode_byte_2 {
                    0x07 => self.data_registers[optcode_nibble_2 as usize] = self.delay_timer,
                    0x0A => {
                        if let Some(key) = self.key_wrapper.get_key() {
                            self.data_registers[optcode_nibble_2 as usize] = key
                        } else {
                            return Ok(());
                        }
                    }
                    0x15 => self.delay_timer = self.data_registers[optcode_nibble_2 as usize],
                    0x18 => self.sound_timer = self.data_registers[optcode_nibble_2 as usize],
                    0x1E => {
                        self.address_register +=
                            self.data_registers[optcode_nibble_2 as usize] as u16
                    }
                    0x29 => {
                        // Font loading
                        self.address_register =
                            self.data_registers[optcode_nibble_2 as usize] as u16 * 5
                    }
                    0x33 => {
                        let nums = self.data_registers[optcode_nibble_2 as usize];
                        self.memory[self.address_register as usize] = nums / 100;
                        self.memory[self.address_register as usize + 1] = nums % 100 / 10;
                        self.memory[self.address_register as usize + 2] = nums % 100 % 10;
                    }
                    0x55 => {
                        for i in 0..optcode_nibble_2 as usize + 1 {
                            self.memory[self.address_register as usize + i] =
                                self.data_registers[i];
                        }
                    }
                    0x65 => {
                        for i in 0..optcode_nibble_2 as usize + 1 {
                            self.data_registers[i] = self.memory[self.address_register as usize +
                                                                 i];
                        }
                    }
                    _ => {
                        return Err("Unimplemented optcode");
                    }
                }
            }
            _ => {
                return Err("Unimplemented optcode");
            }
        }
        self.program_counter += 2;
        Ok(())
    }
    pub fn run_vblank(&mut self) -> Result<(), &str> {
        for _ in 0..11 {
            try!(self.run_optcode())
        }
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }
        if self.sound_timer > 0 {
            self.sound_timer -= 1;
        }
        Ok(())
    }
}
