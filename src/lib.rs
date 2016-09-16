extern crate rand;
extern crate serde;

use rand::Rng;
use std::io::prelude::*;
use std::io::Error;
use std::fmt;
use std::iter::Enumerate;
use std::iter::Iterator;
use std::slice;
use std::mem;
use std::ops::Deref;
use std::ops::DerefMut;
use serde::Serialize;
use serde::Serializer;
use serde::Deserialize;
use serde::Deserializer;
use serde::bytes::ByteBufVisitor;

pub trait KeyWrapper {
    fn is_pushed(&self, u8) -> bool;
    fn get_key(&self) -> Option<u8>;
}

pub trait AudioWrapper {
    fn play(&mut self);
    fn stop(&mut self);
}

struct BitIter<'a> {
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

/// An iterator of all the white pixels returned as (x, y)
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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Chip8Err {
    UnknownOptcode,
    StackUnderFlow,
    BadState,
}

impl fmt::Display for Chip8Err {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Chip8Err::UnknownOptcode => write!(f, "There was an unknown optcode."),
            Chip8Err::StackUnderFlow => write!(f, "There was a stack underflow"),
            Chip8Err::BadState => write!(f, "An invalid state was executed"),
        }
    }
}

static FONT: &'static [u8] = include_bytes!("font.bin");

struct Seriable0x1000Array([u8; 0x1000]);

impl Serialize for Seriable0x1000Array {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error> where S: Serializer {
        serializer.serialize_bytes(&self.0)
    }
}

impl Deserialize for Seriable0x1000Array {
    fn deserialize<D>(deserializer: &mut D)
        -> Result<Seriable0x1000Array, D::Error> where D: Deserializer {
        let mut array = [0; 0x1000];
        let buff = try!(deserializer.deserialize_bytes(ByteBufVisitor));
        array.copy_from_slice(&buff);
        Ok(Seriable0x1000Array(array))
    }
}

include!(concat!(env!("OUT_DIR"), "/serde_types.rs"));

impl Deref for Seriable0x1000Array {
    type Target = [u8; 0x1000];

    fn deref(&self) -> &[u8; 0x1000] {
        &self.0
    }
}

impl DerefMut for Seriable0x1000Array {
    fn deref_mut(&mut self) -> &mut [u8; 0x1000] {
        &mut self.0
    }
}

impl Clone for Chip8State {
    fn clone(&self) -> Chip8State {
        Chip8State {
            data_registers: self.data_registers,
            address_register: self.address_register,
            memory: Seriable0x1000Array(*self.memory),
            program_counter: self.program_counter,
            stack: self.stack.clone(),
            delay_timer: self.delay_timer,
            sound_timer: self.sound_timer,
            frame_buffer: self.frame_buffer,
        }
    }
}

impl Chip8State {
    fn new() -> Chip8State {
        let mut state = Chip8State {
            data_registers: [0; 16],
            address_register: 0,
            memory: Seriable0x1000Array([0; 0x1000]),
            program_counter: 0x200, // Entry point of most programs
            stack: Vec::with_capacity(16),
            delay_timer: 0,
            sound_timer: 0,
            frame_buffer: [[0; 8]; 32],
        };
        state.memory[0..FONT.len()].copy_from_slice(FONT);
        state
    }
    /// Returns a PixelIter over the current screen
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
    pub fn from_prog<T>(input: &mut T) -> Result<Chip8State, Error> where T: Read {
        let mut new_state = Chip8State::new();
        let len = new_state.memory.len();
        {
            let mut program_mem = &mut new_state.memory[0x200..len];
            try!(input.read(program_mem));
        }
        Ok(new_state)
    }
}

/// The chip8 machine
pub struct Chip8<T: KeyWrapper, A: AudioWrapper> {
    pub state: Result<Chip8State, (Option<Chip8State>, Chip8Err)>,
    rng: rand::ThreadRng,
    pub key_wrapper: T,
    pub audio_wrapper: A,
}

impl<T: KeyWrapper, A: AudioWrapper> Chip8<T, A> {
    /// Makes a machine without a state
    pub fn new(key_wrapper: T, audio_wrapper: A) -> Chip8<T, A> {
        Chip8 {
            state: Err((None, Chip8Err::BadState)),
            rng: rand::thread_rng(),
            key_wrapper: key_wrapper,
            audio_wrapper: audio_wrapper,
        }
    }
    fn run_optcode(&mut self) -> Result<(), Chip8Err> {
        let mut state;
        if let Ok(ref mut good_state) = self.state {
            state = good_state
        } else {
            return Err(Chip8Err::BadState)
        }
        let optcode_byte_1 = state.memory[state.program_counter as usize];
        let optcode_nibble_1 = optcode_byte_1 >> 4;
        let optcode_nibble_2 = optcode_byte_1 & 0x0f;
        let optcode_byte_2 = state.memory[state.program_counter as usize + 1];
        let optcode_nibble_3 = optcode_byte_2 >> 4;
        let optcode_nibble_4 = optcode_byte_2 & 0x0f;
        match optcode_nibble_1 {
            0 => {
                if optcode_nibble_2 != 0x00 {
                    return Err(Chip8Err::UnknownOptcode);
                }
                match optcode_byte_2 {
                    0xE0 => state.frame_buffer = [[0; 8]; 32],
                    0xEE => {
                        if let Some(x) = state.stack.pop() {
                            state.program_counter = x;
                            return Ok(());
                        } else {
                            return Err(Chip8Err::StackUnderFlow);
                        }
                    }
                    _ => return Err(Chip8Err::UnknownOptcode),
                }
            }
            1 => {
                state.program_counter = convert_address(optcode_nibble_2, optcode_byte_2);
                return Ok(());
            }
            2 => {
                state.stack.push(state.program_counter + 2);
                state.program_counter = convert_address(optcode_nibble_2, optcode_byte_2);
                return Ok(());
            }
            3 => {
                if state.data_registers[optcode_nibble_2 as usize] == optcode_byte_2 {
                    state.program_counter += 2;
                }
            }
            4 => {
                if state.data_registers[optcode_nibble_2 as usize] != optcode_byte_2 {
                    state.program_counter += 2;
                }
            }
            5 => {
                if state.data_registers[optcode_nibble_4 as usize] != 0 {
                    return Err(Chip8Err::UnknownOptcode);
                }
                if state.data_registers[optcode_nibble_2 as usize] ==
                    state.data_registers[optcode_nibble_3 as usize] {
                    state.program_counter += 2;
                }
            }
            6 => state.data_registers[optcode_nibble_2 as usize] = optcode_byte_2,
            7 => {
                state.data_registers[optcode_nibble_2 as usize] =
                    state.data_registers[optcode_nibble_2 as usize].wrapping_add(optcode_byte_2)
            }
            8 => {
                match optcode_nibble_4 {
                    0 => {
                        state.data_registers[optcode_nibble_2 as usize] =
                            state.data_registers[optcode_nibble_3 as usize]
                    }
                    1 => {
                        state.data_registers[optcode_nibble_2 as usize] |=
                            state.data_registers[optcode_nibble_3 as usize]
                    }
                    2 => {
                        state.data_registers[optcode_nibble_2 as usize] &=
                            state.data_registers[optcode_nibble_3 as usize]
                    }
                    3 => {
                        state.data_registers[optcode_nibble_2 as usize] ^=
                            state.data_registers[optcode_nibble_3 as usize]
                    }
                    4 => {
                        let (added, overflow) = state.data_registers[optcode_nibble_2 as usize]
                            .overflowing_add(state.data_registers[optcode_nibble_3 as usize]);
                        state.data_registers[optcode_nibble_2 as usize] = added;
                        state.data_registers[0xF] = overflow as u8;
                    }
                    5 => {
                        let (subed, mut overflow) = state.data_registers[optcode_nibble_2 as usize]
                            .overflowing_sub(state.data_registers[optcode_nibble_3 as usize]);
                        state.data_registers[optcode_nibble_2 as usize] = subed;
                        overflow = !overflow; // Inverted borrow_flag
                        state.data_registers[0xF] = overflow as u8;
                    }
                    6 => {
                        let lsb = state.data_registers[optcode_nibble_2 as usize] & 1;
                        state.data_registers[optcode_nibble_2 as usize] >>= 1;
                        state.data_registers[0xF] = lsb;
                    }
                    7 => {
                        let (subed, mut overflow) = state.data_registers[optcode_nibble_3 as usize]
                            .overflowing_sub(state.data_registers[optcode_nibble_2 as usize]);
                        state.data_registers[optcode_nibble_2 as usize] = subed;
                        overflow = !overflow; // Inverted borrow_flag
                        state.data_registers[0xF] = overflow as u8;
                    }
                    0xE => {
                        let mut msb = state.data_registers[optcode_nibble_2 as usize] & 0x80;
                        state.data_registers[optcode_nibble_2 as usize] -= msb; // Bypass overflow
                        state.data_registers[optcode_nibble_2 as usize] <<= 1;
                        msb >>= 7; // Move the most significant bit into the least significant bit
                        state.data_registers[0xF] = msb;
                    }
                    _ => return Err(Chip8Err::UnknownOptcode),
                }
            }
            9 => {
                if optcode_nibble_4 != 0 {
                    return Err(Chip8Err::UnknownOptcode);
                }
                if state.data_registers[optcode_nibble_2 as usize] !=
                    state.data_registers[optcode_nibble_3 as usize] {
                    state.program_counter += 2;
                }
            }
            0xA => state.address_register = convert_address(optcode_nibble_2, optcode_byte_2),
            0xB => {
                state.program_counter = convert_address(optcode_nibble_2, optcode_byte_2);
                state.program_counter += state.data_registers[0] as u16;
                return Ok(());
            }
            0xC => {
                let rand: u8 = self.rng.gen();
                state.data_registers[optcode_nibble_2 as usize] = rand & optcode_byte_2;
            }
            0xD => {
                state.data_registers[0xF] = 0;
                for (line_n, line) in state.frame_buffer
                    .iter_mut()
                    .skip(state.data_registers[optcode_nibble_3 as usize] as usize)
                    .take(optcode_nibble_4 as usize)
                    .enumerate() {
                    let mut mut_bit = MutBit::new(line);
                    mut_bit.skip(state.data_registers[optcode_nibble_2 as usize]);
                    for bit in BitIter::new(
                        &state.memory[state.address_register as usize + line_n..state.memory.len()])
                        .take(8) {
                        if bit && mut_bit.toggle() {
                            state.data_registers[0xF] = 1;
                        }
                        mut_bit.next();
                    }
                }
            }
            0xE => {
                match optcode_byte_2 {
                    0x9E => {
                        if self.key_wrapper
                            .is_pushed(state.data_registers[optcode_nibble_2 as usize]) {
                            state.program_counter += 2;
                        }
                    }
                    0xA1 => {
                        if !self.key_wrapper
                            .is_pushed(state.data_registers[optcode_nibble_2 as usize]) {
                            state.program_counter += 2;
                        }
                    }
                    _ => return Err(Chip8Err::UnknownOptcode),
                }
            }
            0xF => {
                match optcode_byte_2 {
                    0x07 => state.data_registers[optcode_nibble_2 as usize] = state.delay_timer,
                    0x0A => {
                        if let Some(key) = self.key_wrapper.get_key() {
                            state.data_registers[optcode_nibble_2 as usize] = key
                        } else {
                            return Ok(());
                        }
                    }
                    0x15 => state.delay_timer = state.data_registers[optcode_nibble_2 as usize],
                    0x18 => {
                        state.sound_timer = state.data_registers[optcode_nibble_2 as usize];
                        if state.sound_timer > 0 {
                            self.audio_wrapper.play();
                        }
                    }
                    0x1E => {
                        state.address_register +=
                            state.data_registers[optcode_nibble_2 as usize] as u16
                    }
                    0x29 => {
                        // Font loading
                        state.address_register =
                            state.data_registers[optcode_nibble_2 as usize] as u16 * 5
                    }
                    0x33 => {
                        let nums = state.data_registers[optcode_nibble_2 as usize];
                        state.memory[state.address_register as usize] = nums / 100;
                        state.memory[state.address_register as usize + 1] = nums % 100 / 10;
                        state.memory[state.address_register as usize + 2] = nums % 100 % 10;
                    }
                    0x55 => {
                        for i in 0..optcode_nibble_2 as usize + 1 {
                            state.memory[state.address_register as usize + i] =
                                state.data_registers[i];
                        }
                    }
                    0x65 => {
                        for i in 0..optcode_nibble_2 as usize + 1 {
                            state.data_registers[i] = state.memory[state.address_register as usize +
                            i];
                        }
                    }
                    _ => return Err(Chip8Err::UnknownOptcode),
                }
            }
            _ => return Err(Chip8Err::UnknownOptcode),
        }
        state.program_counter += 2;
        Ok(())
    }
    fn run_vblank_uncaught(&mut self) -> Result<(), Chip8Err> {
        for _ in 0..11 {
            try!(self.run_optcode())
        }
        let mut state;
        if let Ok(ref mut good_state) = self.state {
            state = good_state
        } else {
            return Err(Chip8Err::BadState)
        }
        if state.delay_timer > 0 {
            state.delay_timer -= 1;
        }
        if state.sound_timer > 0 {
            state.sound_timer -= 1;
            if state.sound_timer == 0 {
                self.audio_wrapper.stop()
            }
        }
        Ok(())
    }
    /// Simulates one frame of a chip8
    pub fn run_vblank(&mut self) -> Result<(), Chip8Err> {
        if let Err(error) = self.run_vblank_uncaught() {
            if error != Chip8Err::BadState {
                let old_state = mem::replace(&mut self.state, Err((None, error))).ok().unwrap();
                self.state = Err((Some(old_state), error));
                self.audio_wrapper.stop()
            }
            Err(error)
        } else {
            Ok(())
        }
    }
    pub fn load_prog<R: Read>(&mut self, input: &mut R) -> Result<(), Error> {
        self.audio_wrapper.stop();
        self.state = Ok(try!(Chip8State::from_prog(input)));
        Ok(())
    }
}

impl<T, K> Clone for Chip8<T, K> where T: Clone + KeyWrapper, K: Clone + AudioWrapper {
    fn clone(&self) -> Chip8<T, K> {
        Chip8 {
            state: self.state.clone(),
            rng: rand::thread_rng(),
            key_wrapper: self.key_wrapper.clone(),
            audio_wrapper: self.audio_wrapper.clone(),
        }
    }
}

/// Panics if the state is invalid
impl<T, K> Deref for Chip8<T, K> where T: KeyWrapper, K: AudioWrapper {
    type Target = Chip8State;

    fn deref(&self) -> &Chip8State {
        self.state.as_ref().ok().expect("Tried to deref an invalid state")
    }
}

impl<T, K> DerefMut for Chip8<T, K> where T: KeyWrapper, K: AudioWrapper {
    fn deref_mut(&mut self) -> &mut Chip8State {
        self.state.as_mut().ok().expect("Tried to deref an invalid state")
    }
}