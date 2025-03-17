use std::fs::File;
use std::io::Read;
use std::io::BufReader;
use std::ops::RemAssign;

const  MAX_16BIT_NUM: u32 = 65_535;
const  MAX_32BIT_NUM: u32 = 4_294_967_295;
const  STACK_SIZE: usize = 512;

fn main() {
    let mut emu_bit_depth = 32;
    // Get user input for emulation depth
    let mut line = String::new();
    println!("Bit depth of emulation? (defualt 16): ");
    let b1 = std::io::stdin().read_line(&mut line).unwrap();
    let mut is_debug = true;

    if emu_bit_depth == 16 {
        emu_16bit();
    }

    if emu_bit_depth == 32 {
        emu_32bit(is_debug);
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

fn emu_32bit(debug: bool){
    let mut emu_ram_32bit: Vec<u32> = vec![0; MAX_32BIT_NUM as usize];
    let mut emu_image_raw: Vec<u8> = vec![]; //vec![0; MAX_16BIT_NUM as usize]; 
    let mut emu_image_32bit: Vec<u32> = vec![];

    // Read in binary File and put it in "disk image"
    let my_buf = BufReader::new(File::open("./output32.bin").unwrap());

    for byte_or_error in my_buf.bytes() {
        let byte = byte_or_error.unwrap();
        emu_image_raw.push(byte);
    }

    println!("{:?}", emu_image_raw);
    let length_32bit = emu_image_raw.len() / 4;
    let mut counter: usize = 0;
    for i in 0..length_32bit {
        let sec1 = (emu_image_raw[counter] as u32) << 24;
        let sec2 = (emu_image_raw[counter + 1] as u32) << 16;
        let sec3 = (emu_image_raw[counter + 2] as u32) << 8;
        let sec4 = emu_image_raw[counter + 3] as u32;
        //println!("{}, {}, {}, {}", sec1, sec2, sec3, sec4);
        emu_image_32bit.push(sec1 + sec2 + sec3 + sec4);
        counter += 4;
    }

    println!("{:?}", emu_image_32bit);

    println!("Starting Emulation");
    let mut emu_running: bool = true;
    let mut current_address: usize = 0;

    //registers
    let mut reg_a: u32 = 0;
    let mut reg_b: u32 = 0;
    let mut reg_c: u32 = 0;
    let mut reg_d: u32 = 0;
    let mut reg_e: u32 = 0;

    //stack 
    let mut emu_stack_pointer: usize = 0;
    let mut emu_stack: [usize; STACK_SIZE] = [0; STACK_SIZE];

    while emu_running { 
        let current_opcode = emu_image_32bit[current_address]; 
        //if debug {println!("Current opcode: {}", current_opcode)};

        match current_opcode {
            0 => {println!("NOP"); continue;}
            1 => {
                if debug {println!("LDA")}
                let target_address = emu_image_32bit[current_address + 1] as usize;
                reg_a = emu_image_32bit[target_address];
                current_address += 2;
                //println!("{}", current_address)
            }
            2 => {
                if debug {println!("LDB")}
                let target_address = emu_image_32bit[current_address + 1] as usize;
                reg_b = emu_image_32bit[target_address];
                current_address += 2;
            }
            3 => {
                if debug {println!("LDD")}
                let target_address = emu_image_32bit[current_address + 1] as usize;
                reg_d = emu_image_32bit[target_address];
                current_address += 2;
            }
            4 => {
                if debug {println!("LDE")}
                let target_address = emu_image_32bit[current_address + 1] as usize;
                reg_e = emu_image_32bit[target_address];
                current_address += 2;
            }
            5 => {
                if debug {println!("STA")}
                let target_address = emu_image_32bit[current_address + 1] as usize;
                emu_ram_32bit[target_address] = reg_a;
                current_address += 2;
            }
            6 => {
                if debug {println!("STB")}
                let target_address = emu_image_32bit[current_address + 1] as usize;
                emu_ram_32bit[target_address] = reg_b;
                current_address += 2;
            }
            7 => {
                if debug {println!("STC")}
                let target_address = emu_image_32bit[current_address + 1] as usize;
                emu_ram_32bit[target_address] = reg_c;
                current_address += 2;
            }
            8 => {
                if debug {println!("STD")}
                let target_address = emu_image_32bit[current_address + 1] as usize;
                emu_ram_32bit[target_address] = reg_d;
                current_address += 2;
            }
            9 => {
                if debug {println!("STE")}
                let target_address = emu_image_32bit[current_address + 1] as usize;
                emu_ram_32bit[target_address] = reg_e;
                current_address += 2;
            }
            10 => {
                if debug {continue};//{println!("PSH")}
                //emu_image_32bit[current_address] = reg_b;
            }
            11 => {
                if debug {println!("PLL")}
                //emu_image_32bit[current_address] = reg_b;
            }
            12 => {
                if debug {println!("ADD")}
                reg_c = reg_a + reg_b;
                current_address += 1
            }
            13 => {
                if debug {println!("SUB")}
                reg_c = reg_a - reg_b;
            }
            14 => {
                if debug {println!("MUL")}
                reg_c = reg_a * reg_b;
            }
            15 => {
                if debug {println!("DIV")}
                reg_c = reg_a / reg_b;
            }
            16 => { //FADD
                if debug {println!("FADD")}
                //reg_c = reg_a + reg_b
            }
            17 => { //FSUB
                if debug {println!("FSUB")}
                //reg_c = reg_a + reg_b
            }
            18 => { //FMUL
                if debug {println!("FMUL")}
                //reg_c = reg_a + reg_b
            }
            19 => { //FDIV
                if debug {println!("FDIV")}
                //reg_c = reg_a + reg_b
            }
            20 => {
                if debug {println!("JMP")}
                let target_address = emu_image_32bit[current_address + 1] as usize;
                emu_stack[emu_stack_pointer] = current_address;
                emu_stack_pointer += 1;
                current_address = target_address;
            }
            21 => {
                if debug {continue}; //{println!("JMPE")}
            }
            22 => {
                if debug {println!("JMPN")}
            }
            23 => {
                if debug {println!("JMPG")}
            }
            24 => {
                if debug {println!("JMPGU")}
            }
            25 => {
                if debug {println!("JMPL")}
            }
            26 => {
                if debug {println!("JMPLU")}
            }
            27 => {
                if debug {println!("CMP")}
            }
            28 => {
                if debug {println!("SHR")}
            }
            29 => {
                if debug {println!("SHL")}
            }
            30 => {
                if debug {println!("AND")}
            }
            31 => {
                if debug {println!("OR")}
            }
            32 => {
                if debug {println!("NOT")}
            }
            33 => {
                if debug {println!("XOR")}
            }
            34 => {
                if debug {println!("NEG")}
            }
            35 => {
                if debug {println!("RET")}
                emu_stack_pointer -= 1;
                current_address = emu_stack[emu_stack_pointer] + 2;
                println!("{}", current_address);
            }
            36 => {
                emu_running = false;
                break;
            }
            _=> {
                println!("ERROR insutrction not regocnised");
                break;
            }
        }

        //current_address += 1;
    }
    println!("Finished Emulation");

    //output registers:
    println!("rega: {}", reg_a);
    println!("regb: {}", reg_b);
    println!("regc: {}", reg_c);
    println!("regd: {}", reg_d);
    println!("rege: {}", reg_e);
    println!("rege: {}", reg_e);
    println!("ram 18: {}", emu_ram_32bit[18]);
}