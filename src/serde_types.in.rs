/// A representation of the chip8 machine's state
#[derive(Serialize, Deserialize)]
pub struct Chip8State {
    data_registers: [u8; 16],
    address_register: u16,
    memory: Seriable0x1000Array,
    program_counter: u16,
    stack: Vec<u16>,
    delay_timer: u8,
    sound_timer: u8,
    frame_buffer: [[u8; 8]; 32],
}