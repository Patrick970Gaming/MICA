//! Config loading, kept compatible with `MICA_inter_emu/config.json` so the
//! two emulators can be pointed at the same file.

use serde_json::Value;
use std::fs::File;
use std::io::BufReader;

pub struct Config {
    pub bit_width: u64,
    pub ram_size: usize,
    pub verbose: bool,
    pub memory_dump_enabled: bool,
    pub memory_dump_start: usize,
    pub memory_dump_length: usize,
    /// JIT-only: block executions before compiling. Absent means 8.
    pub hot_threshold: u32,
    /// JIT-only: print a line per compiled block. Absent means false.
    pub trace_jit: bool,
    /// JIT-only: word address where the code region ends and the constant pool
    /// begins. The image format carries no such boundary, so without this the
    /// engine has to assume the entire image is code and guard every store -
    /// which makes every write to a variable look like self-modifying code and
    /// flushes the block cache. See the design doc's "image header" milestone.
    pub code_limit: Option<u32>,
}

impl Config {
    pub fn load(path: &str) -> Result<Config, String> {
        let file = File::open(path).map_err(|e| format!("{path}: {e}"))?;
        let v: Value =
            serde_json::from_reader(BufReader::new(file)).map_err(|e| format!("{path}: {e}"))?;

        let req_u64 = |key: &str| -> Result<u64, String> {
            v[key]
                .as_u64()
                .ok_or_else(|| format!("{key} missing or not an integer"))
        };

        let bit_width = req_u64("bit_width")?;
        if bit_width != 32 {
            return Err(format!(
                "only bit_width 32 is implemented, config says {bit_width}"
            ));
        }
        let ram_size = req_u64("ram_size")? as usize;
        if !ram_size.is_power_of_two() {
            return Err(format!(
                "ram_size must be a power of two (the JIT masks guest addresses \
                 instead of bounds-checking them); config says {ram_size}"
            ));
        }

        Ok(Config {
            bit_width,
            ram_size,
            verbose: v["verbose"].as_bool().unwrap_or(false),
            memory_dump_enabled: v["memory_dump_enabled"].as_bool().unwrap_or(false),
            memory_dump_start: v["memory_dump_start"].as_u64().unwrap_or(0) as usize,
            memory_dump_length: v["memory_dump_length"].as_u64().unwrap_or(64) as usize,
            hot_threshold: v["hot_threshold"].as_u64().unwrap_or(8) as u32,
            trace_jit: v["trace_jit"].as_bool().unwrap_or(false),
            code_limit: v["code_limit"].as_u64().map(|n| n as u32),
        })
    }
}
