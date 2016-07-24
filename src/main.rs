extern crate sdl2;
extern crate rand;
use std::fs::File;
use sdl2::rect::*;
use sdl2::keyboard::*;
use sdl2::event::*;
use sdl2::EventPump;
mod computer;
use computer::Chip8;
use computer::KeyWrapper;

fn hex_to_scan_code(hex: u8) -> Result<Scancode, &'static str> {
    match hex {
        0 => return Ok(Scancode::X),
        1 => return Ok(Scancode::Num1),
        2 => return Ok(Scancode::Num2),
        3 => return Ok(Scancode::Num3),
        4 => return Ok(Scancode::Q),
        5 => return Ok(Scancode::W),
        6 => return Ok(Scancode::E),
        7 => return Ok(Scancode::A),
        8 => return Ok(Scancode::S),
        9 => return Ok(Scancode::D),
        0xA => return Ok(Scancode::Z),
        0xB => return Ok(Scancode::C),
        0xC => return Ok(Scancode::Num4),
        0xD => return Ok(Scancode::R),
        0xE => return Ok(Scancode::F),
        0xF => return Ok(Scancode::V),
        _ => return Err("Key tested for does not exist"),
    }
}

fn scan_code_to_hex(scancode: Scancode) -> Option<u8> {
    match scancode {
        Scancode::X => return Some(0),
        Scancode::Num1 => return Some(1),
        Scancode::Num2 => return Some(2),
        Scancode::Num3 => return Some(3),
        Scancode::Q => return Some(4),
        Scancode::W => return Some(5),
        Scancode::E => return Some(6),
        Scancode::A => return Some(7),
        Scancode::S => return Some(8),
        Scancode::D => return Some(9),
        Scancode::Z => return Some(0xA),
        Scancode::C => return Some(0xB),
        Scancode::Num4 => return Some(0xC),
        Scancode::R => return Some(0xD),
        Scancode::F => return Some(0xE),
        Scancode::V => return Some(0xF),
        _ => return None,
    }
}

struct SdlKeyStateWrapper(EventPump);

impl KeyWrapper for SdlKeyStateWrapper {
    fn is_pushed(&self, key: u8) -> Result<bool, &'static str> {
        let scancode = try!(hex_to_scan_code(key));
        Ok(self.0.keyboard_state().is_scancode_pressed(scancode))
    }
    fn get_key(&self) -> Option<u8> {
        for key in self.0.keyboard_state().pressed_scancodes() {
            if let Some(hex) = scan_code_to_hex(key) {
                return Some(hex);
            }
        }
        None
    }
}

fn main() {
    let mut args = std::env::args();
    args.next(); // We do not need the path of the executable.
    let sdl = sdl2::init().unwrap();
    let sdl_video = sdl.video().unwrap();
    let sdl_window = sdl_video.window("Chip-8", 64 * 8, 32 * 8)
        .resizable()
        .build()
        .unwrap();
    let sdl_event_pump = sdl.event_pump().unwrap();
    let key_wrap = SdlKeyStateWrapper(sdl_event_pump);
    let mut sdl_renderer = sdl_window.renderer().present_vsync().build().unwrap();
    sdl_renderer.set_logical_size(64, 32).unwrap();
    sdl_renderer.present();
    let chip8 = Chip8::new(key_wrap);
    if let Some(file) = args.next() {
        match File::open(file) {
            Ok(mut input_file) => {
                chip8.load_prog_from_file(&mut input_file).unwrap();
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
    loop {
        if let Err(err) = chip8.run_vblank() {
            println!("{}", err);
            break;
        }
        sdl_renderer.clear();
        for event in chip8.0.borrow_mut().key_wrapper.0.poll_iter() {
            if let Event::Quit { .. } = event {
                return;
            }
        }
        let inner = chip8.0.borrow();
        let frame_buffer = inner.frame_buffer;
        for i in frame_buffer.iter().enumerate() {
            let (y_cord, y) = i;
            for i in y.iter().enumerate() {
                let (x_cord, x) = i;
                if x & 1 != 0 {
                    sdl_renderer.draw_point(Point::new(x_cord as i32, y_cord as i32))
                        .unwrap();
                }
            }
        }
        sdl_renderer.present();
    }
}
