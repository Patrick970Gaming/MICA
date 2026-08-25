//! CLI front end.
//!
//! ```text
//! mica_jit_emu [image.bin] [--config path] [--interp] [--trace] [--diff]
//!                [--code-limit WORDS]
//! ```
//!
//! `--diff` runs the same image on both tiers from a common start state and
//! reports the first divergence; it is the fastest way to find a translation
//! bug during development.

use mica_jit_emu::config::Config;
use mica_jit_emu::cpu::Machine;
use mica_jit_emu::{engine, interp};

fn main() {
    if let Err(err) = real_main() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

struct Args {
    image: String,
    config: String,
    interp_only: bool,
    trace: bool,
    diff: bool,
    code_limit: Option<u32>,
}

fn parse_args() -> Args {
    let mut args = Args {
        image: "./output32.bin".into(),
        config: "./config.json".into(),
        interp_only: false,
        trace: false,
        diff: false,
        code_limit: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--config" => args.config = it.next().unwrap_or_default(),
            "--interp" => args.interp_only = true,
            "--trace" => args.trace = true,
            "--diff" => args.diff = true,
            "--code-limit" => {
                args.code_limit = it.next().and_then(|v| v.parse().ok());
            }
            other => args.image = other.to_string(),
        }
    }
    args
}

fn real_main() -> Result<(), String> {
    let args = parse_args();
    let cfg = Config::load(&args.config)?;
    let image = std::fs::read(&args.image).map_err(|e| format!("{}: {e}", args.image))?;

    let mut m = Machine::new(cfg.ram_size);
    m.load_image(&image)?;
    if let Some(limit) = args.code_limit.or(cfg.code_limit) {
        m.code_limit = limit;
    }

    if cfg.verbose {
        println!(
            "image: {} words, ram: {} words, code region ends at {}, entry: 0",
            image.len() / 4,
            m.ram.len(),
            m.code_limit
        );
    }

    if args.diff {
        return run_diff(&m);
    }

    let (exit, pc) = if args.interp_only {
        let (exit, pc, steps) = interp::run(&mut m, 0, u64::MAX);
        println!("interpreted {steps} instructions");
        (exit, pc)
    } else {
        let mut eng = engine::Engine::new(engine::EngineConfig {
            hot_threshold: cfg.hot_threshold,
            trace: args.trace || cfg.trace_jit,
            ..Default::default()
        })?;
        let result = eng.run(&mut m, 0)?;
        println!("{:?}", eng.stats);
        result
    };

    println!("exit: {exit:?} at pc {pc}");
    println!(
        "a={} b={} c={} d={} e={} sp={}",
        m.cpu.reg_a, m.cpu.reg_b, m.cpu.reg_c, m.cpu.reg_d, m.cpu.reg_e, m.cpu.sp
    );

    if cfg.memory_dump_enabled {
        let end = (cfg.memory_dump_start + cfg.memory_dump_length).min(m.ram.len());
        println!("memory dump [{}..{end}]:", cfg.memory_dump_start);
        for addr in cfg.memory_dump_start..end {
            println!("  [{addr}] = {}", m.ram[addr]);
        }
    }

    Ok(())
}

/// Run both tiers from the same image and compare the final state.
fn run_diff(template: &Machine) -> Result<(), String> {
    let mut a = clone_machine(template);
    let mut b = clone_machine(template);

    let (exit_i, pc_i, steps) = interp::run(&mut a, 0, 50_000_000);

    let mut eng = engine::Engine::new(engine::EngineConfig {
        hot_threshold: 0,
        ..Default::default()
    })?;
    let (exit_j, pc_j) = eng.run(&mut b, 0)?;

    println!("interp: {exit_i:?} @{pc_i} after {steps} insns");
    println!("jit:    {exit_j:?} @{pc_j}  ({:?})", eng.stats);

    if exit_i != exit_j || pc_i != pc_j {
        return Err(format!(
            "divergent exit: {exit_i:?}@{pc_i} vs {exit_j:?}@{pc_j}"
        ));
    }
    if a.cpu != b.cpu {
        return Err(format!(
            "divergent cpu:\n  interp {:?}\n  jit    {:?}",
            a.cpu, b.cpu
        ));
    }
    if let Some(addr) = a.ram.iter().zip(b.ram.iter()).position(|(x, y)| x != y) {
        return Err(format!(
            "divergent ram at {addr}: interp {} vs jit {}",
            a.ram[addr], b.ram[addr]
        ));
    }

    println!(
        "identical: registers, stack and all {} RAM words match",
        a.ram.len()
    );
    Ok(())
}

fn clone_machine(src: &Machine) -> Machine {
    let mut m = Machine::new(src.ram.len());
    m.ram.copy_from_slice(&src.ram);
    m.code_limit = src.code_limit;
    m.cpu = src.cpu.clone();
    m
}
