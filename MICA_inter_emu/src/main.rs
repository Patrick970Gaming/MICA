use std::fs::File;
use std::io::Read;
use std::io::BufReader;

const  MAX_16BIT_NUM: u32 = 65_535;
const  MAX_32BIT_NUM: u32 = 4_294_967_295;

fn main() {
    let mut emu_bit_depth = 16;
    // Get user input for emulation depth
    let mut line = String::new();
    println!("Bit depth of emulation? (defualt 16): ");
    let b1 = std::io::stdin().read_line(&mut line).unwrap();

    if emu_bit_depth == 16 {
        emu_16bit();
    }

    if emu_bit_depth == 32 {
        emu_32bit();
    }
}

fn emu_16bit(){
    let mut emu_ram: Vec<u8> = vec![0; MAX_16BIT_NUM as usize];
    let mut emu_image_16bit: Vec<u8> = vec![]; 

    // Read in binary File and put it in "disk image"
    let my_buf = BufReader::new(File::open("./output16.bin").unwrap());
    for byte_or_error in my_buf.bytes() {
        let byte = byte_or_error.unwrap();
        emu_image_16bit.push(byte);
    }

    println!("{:?}", emu_image_16bit);
}

fn emu_32bit(){
    let mut emu_ram_32bit: Vec<u8> = vec![0; MAX_32BIT_NUM as usize];
    let mut emu_image_32bit: Vec<u8> = vec![]; //vec![0; MAX_16BIT_NUM as usize]; 

    // Read in binary File and put it in "disk image"
    let mut emu_image: Vec<u8> = vec![0; MAX_16BIT_NUM as usize]; 
    let my_buf = BufReader::new(File::open("./output16.bin").unwrap());

    for byte_or_error in my_buf.bytes() {
        let byte = byte_or_error.unwrap();
        emu_image_32bit.push(byte);
    }
}