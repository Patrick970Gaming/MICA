use serde_json::Value;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;

//const MAX_16BIT_NUM: u32 = 65_535;
const MAX_32BIT_NUM: u32 = 4_294_967_295;
const STACK_SIZE: usize = 512;

const INSTRUCTION_SET_FULL: &[&str] = &[
    "NOP", "LDA", "LDAI", "LDB", "LDBI", "LDD", "LDE", "STA", "STAI", "STB", "STBI", "STC", "STD",
    "STE", "PSH", "PLL", "ADD", "SUB", "MUL", "DIV", "FADD", "FSUB", "FMUL", "FDIV", "JMP", "JMPE",
    "JMPN", "JMPG", "JMPGU", "JMPL", "JMPLU", "CMP", "SHR", "SHL", "AND", "OR", "NOT", "XOR",
    "NEG", "RET", "HAL",
];

const INSTRUCTION_SET_FULL_I: &[&str] = &[
    "NOP", "LDA", "LDAI", "LDB", "LDBI", "LDD", "LDE", "STA", "STAI", "STB", "STBI", "STC", "STD",
    "STE", "PSH", "PLL", "ADD", "SUB", "MUL", "DIV", "JMP", "JMPE", "JMPN", "JMPG", "JMPGU",
    "JMPL", "JMPLU", "CMP", "SHR", "SHL", "AND", "OR", "NOT", "XOR", "NEG", "RET", "HAL",
];

fn main() {
    // Read in config
    // let my_buf = BufReader::new(File::open("./output32.bin").unwrap());
    let config_file = File::open("./config.json").unwrap();
    let reader = BufReader::new(config_file);

    let read_config: Value = serde_json::from_reader(reader).unwrap();

    println!(
        "Active config: \n{}",
        serde_json::to_string_pretty(&read_config).expect("Failed to serialize config")
    );

    // Reads in config
    let emu_bitwidth: u64 = read_config["bit_width"]
        .as_u64()
        .expect("bit_width missing or not an integer");

    if !emu_bitwidth.is_power_of_two() {
        panic!("Configured bit width is not a power of two");
    }

    let instruction_standard = read_config["instruction_set_standard"]
        .as_str()
        .expect("instruction_set_standard missing or not a string");

    let instruct_array = match instruction_standard {
        "full" => &INSTRUCTION_SET_FULL,
        "full_int" => &INSTRUCTION_SET_FULL_I,
        _ => panic!("Instruction set standard missing from config"),
    };

    let is_debug = read_config["verbose"]
        .as_bool()
        .expect("verbose missing or not a boolean");

    /*
    if emu_bitwidth == 16 {
        emu_16bit();
    }
    */

    if emu_bitwidth == 32 {
        emu_32bit(is_debug);
    }
}

/*
fn emu_16bit() {
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
*/

fn emu_32bit(debug: bool) {
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
    for _i in 0..length_32bit {
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
    //let mut emu_running: bool = true;
    let mut current_address: usize = 0;

    //registers
    let mut reg_a: u32 = 0;
    let mut reg_b: u32 = 0;
    let mut reg_c: u32 = 0;
    let mut reg_d: u32 = 0;
    let mut reg_e: u32 = 0;

    //stack
    let mut emu_stack_pointer: usize = 0;
    let mut emu_stack: [u32; STACK_SIZE] = [0; STACK_SIZE];

    loop {
        let current_opcode = emu_image_32bit[current_address];
        if debug {
            println!("Current opcode: {}", current_opcode)
        };

        match current_opcode {
            0 => {
                println!("NOP");
                continue;
            }
            1 => {
                if debug {
                    println!("LDA")
                }
                let target_address = emu_image_32bit[current_address + 1] as usize;
                reg_a = emu_image_32bit[target_address];
                current_address += 2;
                //println!("{}", current_address)
            }
            2 => {
                if debug {
                    println!("LDAI")
                }
                let target_address = reg_a as usize;
                reg_b = emu_image_32bit[target_address];
                current_address += 2;
            }
            3 => {
                if debug {
                    println!("LDB")
                }
                let target_address = emu_image_32bit[current_address + 1] as usize;
                reg_b = emu_image_32bit[target_address];
                current_address += 2;
            }
            4 => {
                if debug {
                    println!("LDBI")
                }
                let target_address = reg_b as usize;
                reg_b = emu_image_32bit[target_address];
                current_address += 2;
            }
            5 => {
                if debug {
                    println!("LDD")
                }
                let target_address = emu_image_32bit[current_address + 1] as usize;
                reg_d = emu_image_32bit[target_address];
                current_address += 2;
            }
            6 => {
                if debug {
                    println!("LDE")
                }
                let target_address = emu_image_32bit[current_address + 1] as usize;
                reg_e = emu_image_32bit[target_address];
                current_address += 2;
            }
            7 => {
                if debug {
                    println!("STA")
                }
                let target_address = emu_image_32bit[current_address + 1] as usize;
                emu_ram_32bit[target_address] = reg_a;
                current_address += 2;
            }
            8 => {
                if debug {
                    println!("STAI")
                }
                let target_address = reg_a as usize;
                emu_ram_32bit[target_address] = reg_a;
                current_address += 2;
            }
            9 => {
                if debug {
                    println!("STB")
                }
                let target_address = emu_image_32bit[current_address + 1] as usize;
                emu_ram_32bit[target_address] = reg_b;
                current_address += 2;
            }
            10 => {
                if debug {
                    println!("STBI")
                }
                let target_address = reg_b as usize;
                emu_ram_32bit[target_address] = reg_b;
                current_address += 2;
            }
            11 => {
                if debug {
                    println!("STC")
                }
                let target_address = emu_image_32bit[current_address + 1] as usize;
                emu_ram_32bit[target_address] = reg_c;
                current_address += 2;
            }
            12 => {
                if debug {
                    println!("STD")
                }
                let target_address = emu_image_32bit[current_address + 1] as usize;
                emu_ram_32bit[target_address] = reg_d;
                current_address += 2;
            }
            13 => {
                if debug {
                    println!("STE")
                }
                let target_address = emu_image_32bit[current_address + 1] as usize;
                emu_ram_32bit[target_address] = reg_e;
                current_address += 2;
            }
            14 => {
                if debug {
                    println!("PSH")
                }
                emu_stack[emu_stack_pointer] = reg_a;
                emu_stack_pointer += 1;
                current_address += 1;
            }
            15 => {
                if debug {
                    println!("PLL")
                }
                reg_a = emu_stack[emu_stack_pointer];
                emu_stack_pointer -= 1;
                current_address += 1;
            }
            16 => {
                if debug {
                    println!("ADD")
                }
                reg_c = reg_a + reg_b;
                current_address += 1
            }
            17 => {
                if debug {
                    println!("SUB")
                }
                reg_c = reg_a - reg_b;
            }
            18 => {
                if debug {
                    println!("MUL")
                }
                reg_c = reg_a * reg_b;
            }
            19 => {
                if debug {
                    println!("DIV")
                }
                reg_c = reg_a / reg_b;
            }
            20 => {
                //FADD
                if debug {
                    println!("FADD")
                }
                reg_c = (reg_a as f32 + reg_b as f32) as u32
            }
            21 => {
                //FSUB
                if debug {
                    println!("FSUB")
                }
                reg_c = (reg_a as f32 - reg_b as f32) as u32
            }
            22 => {
                //FMUL
                if debug {
                    println!("FMUL")
                }
                reg_c = (reg_a as f32 * reg_b as f32) as u32
            }
            23 => {
                //FDIV
                if debug {
                    println!("FDIV")
                }
                reg_c = (reg_a as f32 / reg_b as f32) as u32
            }
            24 => {
                if debug {
                    println!("JMP")
                }
                let target_address = emu_image_32bit[current_address + 1] as usize;
                emu_stack[emu_stack_pointer] = current_address as u32;
                emu_stack_pointer += 1;
                current_address = target_address;
            }
            25 => {
                if debug {
                    println!("JMPE")
                }
                if reg_d == 1 {
                    let target_address = emu_image_32bit[current_address + 1] as usize;
                    emu_stack[emu_stack_pointer] = current_address as u32;
                    emu_stack_pointer += 1;
                    current_address = target_address;
                } else {
                };
            }
            26 => {
                if debug {
                    println!("JMPN")
                }
                if reg_d == 2 {
                    let target_address = emu_image_32bit[current_address + 1] as usize;
                    emu_stack[emu_stack_pointer] = current_address as u32;
                    emu_stack_pointer += 1;
                    current_address = target_address;
                } else {
                };
            }
            27 => {
                if debug {
                    println!("JMPG")
                }
                if reg_d == 4 {
                    let target_address = emu_image_32bit[current_address + 1] as usize;
                    emu_stack[emu_stack_pointer] = current_address as u32;
                    emu_stack_pointer += 1;
                    current_address = target_address;
                } else {
                };
            }
            28 => {
                if debug {
                    println!("JMPGU")
                }
                if reg_d == 8 {
                    let target_address = emu_image_32bit[current_address + 1] as usize;
                    emu_stack[emu_stack_pointer] = current_address as u32;
                    emu_stack_pointer += 1;
                    current_address = target_address;
                } else {
                };
            }
            29 => {
                if debug {
                    println!("JMPL")
                }
                if reg_d == 16 {
                    let target_address = emu_image_32bit[current_address + 1] as usize;
                    emu_stack[emu_stack_pointer] = current_address as u32;
                    emu_stack_pointer += 1;
                    current_address = target_address;
                } else {
                };
            }
            30 => {
                if debug {
                    println!("JMPLU")
                }
                if reg_d == 32 {
                    let target_address = emu_image_32bit[current_address + 1] as usize;
                    emu_stack[emu_stack_pointer] = current_address as u32;
                    emu_stack_pointer += 1;
                    current_address = target_address;
                } else {
                };
            }
            31 => {
                if debug {
                    println!("CMP")
                }
                if reg_a == reg_b {
                    reg_d = 1; // set flag register to equals
                } else if reg_a != reg_b {
                    reg_d = 2; // set flag register to not equal (Signed)
                } else if (reg_a as i32) > (reg_b as i32) {
                    reg_d = 4; // set flag register to greater than (signed)
                } else if reg_a > reg_b {
                    reg_d = 8; // set flag register to greater than (unsigned)
                } else if (reg_a as i32) < (reg_b as i32) {
                    reg_d = 16; // set flag register to Less than (signed)
                } else if reg_a < reg_b {
                    reg_d = 32; // set flag register to less than (unsigned)
                }
            }
            32 => {
                if debug {
                    println!("SHR")
                }
                reg_c = reg_a >> 1
            }
            33 => {
                if debug {
                    println!("SHL")
                }
                reg_c = reg_a << 1
            }
            34 => {
                if debug {
                    println!("AND")
                }
                reg_c = reg_a & reg_b;
            }
            35 => {
                if debug {
                    println!("OR")
                }
                reg_c = reg_a | reg_b;
            }
            36 => {
                if debug {
                    println!("NOT")
                }
                reg_c = !reg_a;
            }
            37 => {
                if debug {
                    println!("XOR")
                }
                reg_c = reg_a ^ reg_b;
            }
            38 => {
                if debug {
                    println!("NEG")
                }
                reg_c = reg_a.wrapping_neg()
            }
            39 => {
                if debug {
                    println!("RET")
                }
                emu_stack_pointer -= 1;
                current_address = (emu_stack[emu_stack_pointer] + 2) as usize;
                println!("{}", current_address);
            }
            40 => {
                if debug {
                    println!("HALT")
                }
                //emu_running = false;
                break;
            }
            _ => {
                println!("ERROR insutrction not regocnised");
                break;
            }
        }
        //thread::sleep(time::Duration::from_millis(100));
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
