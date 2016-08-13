extern crate rand;

use rand::Rng;
use std::fs::File;
use std::io::prelude::*;
use std::io::Error;
use std::fmt;
use std::iter::Enumerate;
use std::iter::Iterator;
use std::slice;

pub trait KeyWrapper {
    fn is_pushed(&self, u8) -> bool;
    fn get_key(&self) -> Option<u8>;
}

pub trait AudioWrapper {
    fn play(&mut self);
    fn stop(&mut self);
}

pub struct Chip8<T: KeyWrapper, A: AudioWrapper> {
    data_registers: [u8; 16],
    address_register: u16,
    memory: [u8; 0x1000],
    program_counter: u16,
    stack: Vec<u16>,
    delay_timer: u8,
    sound_timer: u8,
    frame_buffer: [[u8; 8]; 32],
    // Byte == zero black, byte == one white
    rng: rand::ThreadRng,
    pub key_wrapper: T,
    pub audio_wrapper: A,
}

pub struct BitIter<'a> {
    slice_iter: std::slice::Iter<'a, u8>,
    current_byte: u8,
    bit_mask: u8,
}

impl<'a> BitIter<'a> {
    fn new(slice: &[u8]) -> BitIter {
        let mut iter = slice.iter();
        let current_byte;
        let bit_mask;
        if let Some(byte) = iter.next() {
            current_byte = *byte;
            bit_mask = 1 << 7;
        } else {
            current_byte = 0;
            bit_mask = 0;
        }
        BitIter {
            slice_iter: iter,
            current_byte: current_byte,
            bit_mask: bit_mask,
        }
    }
}

impl<'a> Iterator for BitIter<'a> {
    type Item = bool;

    fn next(&mut self) -> Option<bool> {
        if self.bit_mask > 0 {
            let return_val = self.current_byte & self.bit_mask != 0;
            self.bit_mask >>= 1;
            Some(return_val)
        } else if let Some(byte) = self.slice_iter.next() {
            self.current_byte = *byte;
            self.bit_mask = 1 << 7;
            self.next()
        } else {
            None
        }
    }
}

struct MutBit<'a> {
    slice: &'a mut [u8],
    index: usize,
    bit_mask: u8,
}

impl<'a> MutBit<'a> {
    fn new(slice: &mut [u8]) -> MutBit {
        let bit_mask = 1 << 7;
        MutBit {
            slice: slice,
            index: 0,
            bit_mask: bit_mask,
        }
    }

    fn next(&mut self) {
        self.bit_mask >>= 1;
        if self.bit_mask == 0 {
            self.bit_mask = 1 << 7;
            self.index += 1;
            if self.index >= self.slice.len() {
                self.index = 0;
            }
        }
    }

    fn toggle(&mut self) -> bool {
        let return_val = self.slice[self.index] & self.bit_mask != 0;
        self.slice[self.index] ^= self.bit_mask;
        return_val
    }

    fn skip(&mut self, skip: u8) {
        for _ in 0..skip {
            self.next()
        }
    }
}

pub struct PixelIter<'a> {
    itery: Enumerate<slice::Iter<'a, [u8; 8]>>,
    y_pos: usize,
    iterx: Enumerate<BitIter<'a>>,
}

impl<'a> Iterator for PixelIter<'a> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<(usize, usize)> {
        // x, y
        loop {
            if let Some((x, pixel)) = self.iterx.next() {
                if pixel {
                    return Some((x, self.y_pos));
                }
                continue;
            }
            if let Some((y, iter_x)) = self.itery.next() {
                self.iterx = BitIter::new(iter_x).enumerate();
                self.y_pos = y;
                continue;
            }
            return None;
        }
    }
}

fn convert_address(nibble: u8, byte: u8) -> u16 {
    let mut address = nibble as u16;
    address <<= 8;
    address | byte as u16
}

static FONT: &'static [u8] = include_bytes!("font.bin");

pub enum Chip8Err {
    UnknownOptcode,
    StackUnderFlow,
}

impl fmt::Display for Chip8Err {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Chip8Err::UnknownOptcode => write!(f, "There was an unknown optcode."),
            Chip8Err::StackUnderFlow => write!(f, "There was a stack underflow"),
        }
    }
}

impl<T: KeyWrapper, A: AudioWrapper> Chip8<T, A> {
    pub fn new(key_wrapper: T, audio_wrapper: A) -> Chip8<T, A> {
        let mut chip8 = Chip8 {
            data_registers: [0; 16],
            address_register: 0,
            memory: [0; 0x1000],
            program_counter: 0x200, // Entry point of most programs
            stack: Vec::with_capacity(16),
            delay_timer: 0,
            sound_timer: 0,
            frame_buffer: [[0; 8]; 32],
            rng: rand::thread_rng(),
            key_wrapper: key_wrapper,
            audio_wrapper: audio_wrapper,
        };
        chip8.memory[0..FONT.len()].copy_from_slice(FONT);
        chip8
    }
    pub fn load_prog_from_file(&mut self, input_file: &mut File) -> Result<usize, Error> {
        let len = self.memory.len();
        let mut program_mem = &mut self.memory[0x200..len];
        input_file.read(program_mem)
    }
    pub fn run_optcode(&mut self) -> Result<(), Chip8Err> {
        let optcode_byte_1 = self.memory[self.program_counter as usize];
        let optcode_nibble_1 = optcode_byte_1 >> 4;
        let optcode_nibble_2 = optcode_byte_1 & 0x0f;
        let optcode_byte_2 = self.memory[self.program_counter as usize + 1];
        let optcode_nibble_3 = optcode_byte_2 >> 4;
        let optcode_nibble_4 = optcode_byte_2 & 0x0f;
        match optcode_nibble_1 {
            0 => {
                if optcode_nibble_2 != 0x00 {
                    return Err(Chip8Err::UnknownOptcode)
                }
                match optcode_byte_2 {
                    0xE0 => self.frame_buffer = [[0; 8]; 32],
                    0xEE => {
                        if let Some(x) = self.stack.pop() {
                            self.program_counter = x;
                            return Ok(());
                        } else {
                            return Err(Chip8Err::StackUnderFlow);
                        }
                    }
                    _ => {
                        return Err(Chip8Err::UnknownOptcode)
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
                    return Err(Chip8Err::UnknownOptcode)
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
                        return Err(Chip8Err::UnknownOptcode)
                    }
                }
            }
            9 => {
                if optcode_nibble_4 != 0 {
                    return Err(Chip8Err::UnknownOptcode)
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
                for (line_n, line) in self.frame_buffer.iter_mut()
                    .skip(self.data_registers[optcode_nibble_3 as usize] as usize)
                    .take(optcode_nibble_4 as usize).enumerate() {
                    let mut mut_bit = MutBit::new(line);
                    mut_bit.skip(self.data_registers[optcode_nibble_2 as usize]);
                    for bit in BitIter::new(
                        &self.memory[self.address_register as usize + line_n..self.memory.len()])
                        .take(8) {
                        if bit && mut_bit.toggle() {
                            self.data_registers[0xF] = 1;
                        }
                        mut_bit.next();
                    }
                }
            }
            0xE => {
                match optcode_byte_2 {
                    0x9E => {
                        if self.key_wrapper
                            .is_pushed(self.data_registers[optcode_nibble_2 as usize]) {
                            self.program_counter += 2;
                        }
                    }
                    0xA1 => {
                        if !self.key_wrapper
                            .is_pushed(self.data_registers[optcode_nibble_2 as usize]) {
                            self.program_counter += 2;
                        }
                    }
                    _ => {
                        return Err(Chip8Err::UnknownOptcode)
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
                    0x18 => {
                        self.sound_timer = self.data_registers[optcode_nibble_2 as usize];
                        if self.sound_timer > 0 {
                            self.audio_wrapper.play();
                        }
                    }
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
                        return Err(Chip8Err::UnknownOptcode)
                    }
                }
            }
            _ => {
                return Err(Chip8Err::UnknownOptcode)
            }
        }
        self.program_counter += 2;
        Ok(())
    }
    pub fn run_vblank(&mut self) -> Result<(), Chip8Err> {
        for _ in 0..11 {
            try!(self.run_optcode())
        }
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }
        if self.sound_timer > 0 {
            self.sound_timer -= 1;
            if self.sound_timer == 0 {
                self.audio_wrapper.stop()
            }
        }
        Ok(())
    }
    pub fn reboot(&mut self) {
        self.data_registers.copy_from_slice(&[0; 16]);
        self.address_register = 0;
        self.memory.copy_from_slice(&[0; 0x1000]);
        self.program_counter = 0x200; // Entry point of most programs
        self.stack.clear();
        self.delay_timer = 0;
        self.sound_timer = 0;
        self.frame_buffer = [[0; 8]; 32];
        self.memory[0..FONT.len()].copy_from_slice(FONT);
    }
    pub fn frame_iter(&self) -> PixelIter {
        let mut itery = self.frame_buffer.iter().enumerate();
        let (y_pos, x) = itery.next().unwrap();
        let iterx = BitIter::new(x).enumerate();
        PixelIter {
            itery: itery,
            y_pos: y_pos,
            iterx: iterx,
        }
    }
}
