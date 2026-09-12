use serde_json::Value;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::time::Instant;
use yansi::Paint;

//const MAX_16BIT_NUM: u32 = 65_535;
const MAX_32BIT_NUM: u32 = 4_294_967_295;
const STACK_SIZE: usize = 512;

const INSTRUCTION_SET_FULL: &[&str] = &[
    "NOP", "LDA", "LDAI", "LDB", "LDBI", "LDC", "LDD", "LDE", "STA", "STAI", "STB", "STBI", "STC",
    "STD", "STE", "PSH", "PLL", "ADD", "SUB", "MUL", "DIV", "FADD", "FSUB", "FMUL", "FDIV", "JMP",
    "JMPE", "JMPN", "JMPG", "JMPGU", "JMPL", "JMPLU", "JMPI", "JMPEI", "JMPNI", "JMPGI", "JMPGUI",
    "JMPLI", "JMPLUI", "CMP", "SHR", "SHL", "AND", "OR", "NOT", "XOR", "NEG", "RET", "HAL",
];

const INSTRUCTION_SET_FULL_I: &[&str] = &[
    "NOP", "LDA", "LDAI", "LDB", "LDBI", "LDC", "LDD", "LDE", "STA", "STAI", "STB", "STBI", "STC",
    "STD", "STE", "PSH", "PLL", "ADD", "SUB", "MUL", "DIV", "JMP", "JMPE", "JMPN", "JMPG", "JMPGU",
    "JMPL", "JMPLU", "JMPI", "JMPEI", "JMPNI", "JMPGI", "JMPGUI", "JMPLI", "JMPLUI", "CMP", "SHR",
    "SHL", "AND", "OR", "NOT", "XOR", "NEG", "RET", "HAL",
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

    let ram_size: u64 = read_config["ram_size"]
        .as_u64()
        .expect("ram_size missing or not an integer");
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

    let memory_dump_enabled = read_config["memory_dump_enabled"]
        .as_bool()
        .expect("memory_dump_enabled missing or not a bool");

    let memory_dump_start: u64 = read_config["memory_dump_start"]
        .as_u64()
        .expect("memory_dump_start missing or not an integer");

    let memory_dump_length: u64 = read_config["memory_dump_length"]
        .as_u64()
        .expect("memory_dump_start missing or not an integer");

    let rom_start: u64 = read_config["program_rom_start"]
        .as_u64()
        .expect("memory_dump_start missing or not an integer");

    let bios_start: u64 = read_config["bios_start"]
        .as_u64()
        .expect("memory_dump_start missing or not an integer");

    let benchmark = read_config["benchmark"]
        .as_bool()
        .expect("memory_dump_start missing or not an integer");
    /*
    if emu_bitwidth == 16 {
        emu_16bit();
    }
    */

    if emu_bitwidth == 32 {
        emu_32bit(
            is_debug,
            ram_size as usize,
            memory_dump_enabled,
            memory_dump_start as usize,
            memory_dump_length as usize,
            rom_start as usize,
            bios_start as usize,
            benchmark as bool,
        );
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

fn load_image_into_mmio(
    path: &str,
    ram: &mut [u32],
    debug: bool,
    start_addr: usize,
    end_addr: usize,
) {
    let my_buf = BufReader::new(File::open(path).unwrap());

    let mut emu_image_raw: Vec<u8> = vec![];
    for byte_or_error in my_buf.bytes() {
        emu_image_raw.push(byte_or_error.unwrap());
    }
    if debug {
        println!("{:?}", emu_image_raw);
    }

    let length_32bit = emu_image_raw.len() / 4;

    // Sanity check the bounds themselves before using them.
    if end_addr < start_addr {
        panic!(
            "Invalid memory range: end_addr {} is before start_addr {}",
            end_addr, start_addr
        );
    }
    if end_addr > ram.len() {
        panic!(
            "end_addr {} exceeds ram_size {} - increase ram_size or adjust end_addr in config.json",
            end_addr,
            ram.len()
        );
    }

    let region_size = end_addr - start_addr;
    if length_32bit > region_size {
        panic!(
            "Program requires {} words of memory but the region [{}, {}) only has room for {} - increase the region or reduce the program size",
            length_32bit, start_addr, end_addr, region_size
        );
    }

    let mut counter: usize = 0;
    for word in ram[start_addr..end_addr].iter_mut().take(length_32bit) {
        let sec1 = (emu_image_raw[counter] as u32) << 24;
        let sec2 = (emu_image_raw[counter + 1] as u32) << 16;
        let sec3 = (emu_image_raw[counter + 2] as u32) << 8;
        let sec4 = emu_image_raw[counter + 3] as u32;
        *word = sec1 + sec2 + sec3 + sec4;
        counter += 4;
    }

    //println!("{:?}", ram);
}

fn emu_32bit(
    debug: bool,
    ram_size: usize,
    memory_dump_enabled: bool,
    memory_dump_start: usize,
    memory_dump_length: usize,
    rom_start: usize,
    bios_start: usize,
    benchmark: bool,
) {
    let mut emu_mmio_32bit: Vec<u32> = vec![0; ram_size as usize];
    load_image_into_mmio(
        "./output32.bin",
        &mut emu_mmio_32bit,
        debug,
        rom_start,
        ram_size,
    );

    load_image_into_mmio(
        "./micabios32.bin",
        &mut emu_mmio_32bit,
        debug,
        bios_start,
        rom_start,
    );

    println!("Starting Emulation");
    //let mut emu_running: bool = true;
    let mut current_address: usize = bios_start;

    //registers
    let mut reg_a: u32 = 0;
    let mut reg_b: u32 = 0;
    let mut reg_c: u32 = 0;
    let mut reg_d: u32 = 0;
    let mut reg_e: u32 = 0;

    //stack
    let mut emu_stack_pointer: usize = 0;
    let mut emu_stack: [u32; STACK_SIZE] = [0; STACK_SIZE];

    //benchmark varibles
    let mut cycle_count: u64 = 0;
    let start = Instant::now();

    loop {
        if current_address >= ram_size {
            println!("{}", Paint::red("Programed excedeed ram size"));
            break;
        }
        let current_opcode = emu_mmio_32bit[current_address];
        if debug {
            println!("Current opcode: {}", current_opcode)
        };

        match current_opcode {
            0 => {
                if debug {
                    println!("NOP")
                }
                current_address += 1;
            }
            1 => {
                if debug {
                    println!("LDA")
                }
                let target_address = emu_mmio_32bit[current_address + 1] as usize;
                reg_a = emu_mmio_32bit[target_address];
                current_address += 2;
                if debug {
                    println!("reg_a = {}", reg_a);
                    println!("Target address: {}", target_address);
                    println!("Data at target address: {}", target_address);
                }
            }
            2 => {
                if debug {
                    println!("LDAI")
                }
                let target_address = reg_c as usize;
                reg_a = emu_mmio_32bit[target_address];
                current_address += 1;
                if debug {
                    println!("reg_a = {}", reg_a);
                    println!("Target address: {}", target_address);
                    println!("Data at target address: {}", emu_mmio_32bit[target_address]);
                }
            }
            3 => {
                if debug {
                    println!("LDB")
                }
                let target_address = emu_mmio_32bit[current_address + 1] as usize;
                reg_b = emu_mmio_32bit[target_address];
                current_address += 2;
                if debug {
                    println!("Target address: {}", target_address);
                    println!("Data at target address: {}", emu_mmio_32bit[target_address]);
                };
            }
            4 => {
                if debug {
                    println!("LDBI")
                }
                let target_address = reg_c as usize;
                reg_b = emu_mmio_32bit[target_address];
                current_address += 1;
            }
            5 => {
                if debug {
                    println!("LDC")
                }
                let target_address = emu_mmio_32bit[current_address + 1] as usize;
                reg_c = emu_mmio_32bit[target_address];
                current_address += 2;
                if debug {
                    println!("reg_c = {}", reg_c);
                    println!("Target address: {}", target_address);
                    println!("Data at target address: {}", emu_mmio_32bit[target_address]);
                };
            }
            6 => {
                if debug {
                    println!("LDD")
                }
                let target_address = emu_mmio_32bit[current_address + 1] as usize;
                reg_d = emu_mmio_32bit[target_address];
                current_address += 2;
            }
            7 => {
                if debug {
                    println!("LDE")
                }
                let target_address = emu_mmio_32bit[current_address + 1] as usize;
                reg_e = emu_mmio_32bit[target_address];
                current_address += 2;
            }
            8 => {
                if debug {
                    println!("STA")
                }
                let target_address = emu_mmio_32bit[current_address + 1] as usize;
                emu_mmio_32bit[target_address] = reg_a;
                current_address += 2;
                if debug {
                    println!("reg_a = {}", reg_a);
                    println!("Target address: {}", target_address);
                    println!("Data at target address: {}", emu_mmio_32bit[target_address]);
                }
            }
            9 => {
                if debug {
                    println!("STAI")
                }
                let target_address = reg_c as usize;
                emu_mmio_32bit[target_address] = reg_a;
                current_address += 1;
                if debug {
                    println!("reg_a = {}", reg_a);
                    println!("Target address: {}", target_address);
                    println!("Data at target address: {}", emu_mmio_32bit[target_address]);
                }
            }
            10 => {
                if debug {
                    println!("STB")
                }
                let target_address = emu_mmio_32bit[current_address + 1] as usize;
                emu_mmio_32bit[target_address] = reg_b;
                current_address += 2;
            }
            11 => {
                if debug {
                    println!("STBI")
                }
                let target_address = reg_c as usize;
                emu_mmio_32bit[target_address] = reg_b;
                current_address += 1;
            }
            12 => {
                if debug {
                    println!("STC")
                }
                let target_address = emu_mmio_32bit[current_address + 1] as usize;
                emu_mmio_32bit[target_address] = reg_c;
                current_address += 2;
                if debug {
                    println!("reg_c = {}", reg_c);
                    println!("Target address: {}", target_address);
                    println!("Data at target address: {}", emu_mmio_32bit[target_address]);
                };
            }
            13 => {
                if debug {
                    println!("STD")
                }
                let target_address = emu_mmio_32bit[current_address + 1] as usize;
                emu_mmio_32bit[target_address] = reg_d;
                current_address += 2;
            }
            14 => {
                if debug {
                    println!("STE")
                }
                let target_address = emu_mmio_32bit[current_address + 1] as usize;
                emu_mmio_32bit[target_address] = reg_e;
                current_address += 2;
            }
            15 => {
                if debug {
                    println!("PSH")
                }
                emu_stack[emu_stack_pointer] = reg_a;
                emu_stack_pointer += 1;
                current_address += 1;
            }
            16 => {
                if debug {
                    println!("PLL")
                }
                emu_stack_pointer -= 1;
                reg_a = emu_stack[emu_stack_pointer];
                current_address += 1;
            }
            17 => {
                if debug {
                    println!("ADD")
                }
                reg_c = reg_a + reg_b;
                current_address += 1
            }
            18 => {
                if debug {
                    println!("SUB")
                }
                reg_c = reg_a - reg_b;
                current_address += 1;
            }
            19 => {
                if debug {
                    println!("MUL")
                }
                reg_c = reg_a * reg_b;
                current_address += 1;
            }
            20 => {
                if debug {
                    println!("DIV")
                }
                reg_c = reg_a / reg_b;
                current_address += 1;
            }
            21 => {
                //FADD
                if debug {
                    println!("FADD")
                }
                let result = f32::from_bits(reg_a) + f32::from_bits(reg_b);
                reg_c = result.to_bits();
                current_address += 1;
            }
            22 => {
                //FSUB
                if debug {
                    println!("FSUB")
                }
                let result = f32::from_bits(reg_a) - f32::from_bits(reg_b);
                reg_c = result.to_bits();
                current_address += 1;
            }
            23 => {
                //FMUL
                if debug {
                    println!("FMUL")
                }
                let result = f32::from_bits(reg_a) * f32::from_bits(reg_b);
                reg_c = result.to_bits();
                current_address += 1;
            }
            24 => {
                //FDIV
                if debug {
                    println!("FDIV")
                }
                let result = f32::from_bits(reg_a) / f32::from_bits(reg_b);
                reg_c = result.to_bits();
                current_address += 1;
            }
            25 => {
                if debug {
                    println!("JMP")
                }
                let target_address = emu_mmio_32bit[current_address + 1] as usize;
                emu_stack[emu_stack_pointer] = current_address as u32;
                emu_stack_pointer += 1;
                current_address = target_address;
            }
            26 => {
                if debug {
                    println!("JMPE")
                }
                if reg_d & 1 != 0 {
                    let target_address = emu_mmio_32bit[current_address + 1] as usize;
                    current_address = target_address;
                } else {
                    current_address += 2;
                };
            }
            27 => {
                if debug {
                    println!("JMPN")
                }
                if reg_d & 1 == 0 {
                    let target_address = emu_mmio_32bit[current_address + 1] as usize;
                    current_address = target_address;
                } else {
                    current_address += 2;
                };
            }
            28 => {
                if debug {
                    println!("JMPG")
                }
                if reg_d & 2 != 0 {
                    let target_address = emu_mmio_32bit[current_address + 1] as usize;
                    current_address = target_address;
                } else {
                    current_address += 2;
                };
            }
            29 => {
                if debug {
                    println!("JMPGU")
                }
                if reg_d & 8 != 0 {
                    let target_address = emu_mmio_32bit[current_address + 1] as usize;
                    current_address = target_address;
                } else {
                    current_address += 2;
                };
            }
            30 => {
                if debug {
                    println!("JMPL")
                }
                if reg_d & 4 != 0 {
                    let target_address = emu_mmio_32bit[current_address + 1] as usize;
                    current_address = target_address;
                } else {
                    current_address += 2;
                };
            }
            31 => {
                if debug {
                    println!("JMPLU")
                }
                if reg_d & 16 != 0 {
                    let target_address = emu_mmio_32bit[current_address + 1] as usize;
                    current_address = target_address;
                } else {
                    current_address += 2;
                };
            }
            32 => {
                if debug {
                    println!("JMPI")
                }
                let target_address = reg_c as usize;
                // JMPI is a 1-word instruction (no operand word, unlike JMP's 2),
                // but RET always does `popped + 2` to skip a call's opcode+operand.
                // Push current_address - 1 so that RET's fixed +2 lands on
                // current_address + 1, i.e. the instruction right after JMPI.
                emu_stack[emu_stack_pointer] = (current_address as u32).wrapping_sub(1);
                emu_stack_pointer += 1;
                current_address = target_address;
            }
            33 => {
                if debug {
                    println!("JMPEI")
                }
                if reg_d & 1 != 0 {
                    current_address = reg_c as usize;
                } else {
                    current_address += 1;
                };
            }
            34 => {
                if debug {
                    println!("JMPNI")
                }
                if reg_d & 1 == 0 {
                    current_address = reg_c as usize;
                } else {
                    current_address += 1;
                };
            }
            35 => {
                if debug {
                    println!("JMPGI")
                }
                if reg_d & 2 != 0 {
                    current_address = reg_c as usize;
                } else {
                    current_address += 1;
                };
            }
            36 => {
                if debug {
                    println!("JMPGUI")
                }
                if reg_d & 8 != 0 {
                    current_address = reg_c as usize;
                } else {
                    current_address += 1;
                };
            }
            37 => {
                if debug {
                    println!("JMPLI")
                }
                if reg_d & 4 != 0 {
                    current_address = reg_c as usize;
                } else {
                    current_address += 1;
                };
            }
            38 => {
                if debug {
                    println!("JMPLUI")
                }
                if reg_d & 16 != 0 {
                    current_address = reg_c as usize;
                } else {
                    current_address += 1;
                };
            }
            39 => {
                if debug {
                    println!("CMP")
                }
                let mut flags: u32 = 0;
                if reg_a == reg_b {
                    flags |= 1; // EQ
                } else {
                    if (reg_a as i32) > (reg_b as i32) {
                        flags |= 2;
                    } else {
                        flags |= 4;
                    } // signed GT / LT
                    if reg_a > reg_b {
                        flags |= 8;
                    } else {
                        flags |= 16;
                    } // unsigned GT / LT
                }
                reg_d = flags;
                current_address += 1;
            }
            40 => {
                if debug {
                    println!("SHR")
                }
                reg_c = reg_a >> 1;
                current_address += 1;
            }
            41 => {
                if debug {
                    println!("SHL")
                }
                reg_c = reg_a << 1;
                current_address += 1;
            }
            42 => {
                if debug {
                    println!("AND")
                }
                reg_c = reg_a & reg_b;
                current_address += 1;
            }
            43 => {
                if debug {
                    println!("OR")
                }
                reg_c = reg_a | reg_b;
                current_address += 1;
            }
            44 => {
                if debug {
                    println!("NOT")
                }
                reg_c = !reg_a;
                current_address += 1;
            }
            45 => {
                if debug {
                    println!("XOR")
                }
                reg_c = reg_a ^ reg_b;
                current_address += 1;
            }
            46 => {
                if debug {
                    println!("NEG")
                }
                reg_c = reg_a.wrapping_neg();
                current_address += 1;
            }
            47 => {
                if debug {
                    println!("RET");
                    println!("{}", current_address);
                }
                emu_stack_pointer -= 1;
                current_address = (emu_stack[emu_stack_pointer] + 2) as usize;
            }
            48 => {
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
        if benchmark {
            cycle_count += 1;
        }
    }

    let elapsed = start.elapsed();

    if benchmark {
        let cycles_per_sec = cycle_count as f64 / elapsed.as_secs_f64();
        println!(
            "Cycles per second: {}",
            format_cycles_per_sec(cycles_per_sec)
        );
    }

    println!("Finished Emulation");

    if memory_dump_enabled {
        let dump_end = (memory_dump_start + memory_dump_length).min(emu_mmio_32bit.len());
        println!("Memory dump [{}..{}]:", memory_dump_start, dump_end);
        for addr in memory_dump_start..dump_end {
            println!("  [{}] = {}", addr, emu_mmio_32bit[addr]);
        }
    }

    //output registers:
    println!("rega: {}", reg_a);
    println!("regb: {}", reg_b);
    println!("regc: {}", reg_c);
    println!("regd: {}", reg_d);
    println!("rege: {}", reg_e);
    println!("rege: {}", reg_e);
}

fn format_cycles_per_sec(cycles_per_sec: f64) -> String {
    if cycles_per_sec >= 1_000_000_000.0 {
        format!("{:.2} B/s", cycles_per_sec / 1_000_000_000.0)
    } else if cycles_per_sec >= 1_000_000.0 {
        format!("{:.2} M/s", cycles_per_sec / 1_000_000.0)
    } else if cycles_per_sec >= 1_000.0 {
        format!("{:.2} K/s", cycles_per_sec / 1_000.0)
    } else {
        format!("{:.2} /s", cycles_per_sec)
    }
}
