#![allow(unused)]

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;

use std::time::{Instant, Duration};
use std::process::{Command, ExitStatus};
use structopt::StructOpt;
use std::env;
use std::path::{PathBuf, Path};
use std::fs::{self, File};
use std::iter;
use std::io::Write;
use anyhow::{Result, bail};

#[derive(Debug, StructOpt)]
struct Options {
    #[structopt(subcommand)]
    cmd: Cmd,
    #[structopt(flatten)]
    global: GlobalOptions,
}

#[derive(Debug, StructOpt)]
enum Cmd {
    GenOneCase {
        num_types: u32,
        num_calls: u32,
    },
    CompileOneCase {
        num_types: u32,
        num_calls: u32,
    },
    RunOneCase {
        num_types: u32,
        num_calls: u32,
    },
    GenAllCases {
        num_types: u32,
        num_calls: u32,
        step: u32,
    },
    CompileAllCases {
        num_types: u32,
        num_calls: u32,
        step: u32,
    },
    RunAllCases {
        num_types: u32,
        num_calls: u32,
        step: u32,
    },
}

#[derive(Debug, StructOpt)]
struct GlobalOptions {
    #[structopt(default_value = "cases", long = "outdir")]
    outdir: PathBuf,
}

fn main() -> Result<()> {
    let options = Options::from_args();

    match options.cmd {
        Cmd::GenOneCase { num_types, num_calls } => {
            let config = CaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_calls
            };
            gen_one_case(config)?;
        }
        Cmd::CompileOneCase { num_types, num_calls } => {
            let config = CaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_calls
            };
            compile_one_case(config)?;
        }
        Cmd::RunOneCase { num_types, num_calls } => {
            let config = CaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_calls
            };
            run_one_case(config)?;
        }
        Cmd::GenAllCases { num_types, num_calls, step } => {
            let config = CaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_calls
            };
            gen_all_cases(config, step)?;
        }
        Cmd::CompileAllCases { num_types, num_calls, step } => {
            let config = CaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_calls
            };
            compile_all_cases(config, step)?;
        }
        Cmd::RunAllCases { num_types, num_calls, step } => {
            let config = CaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_calls
            };
            run_all_cases(config, step)?;
        }
    }

    Ok(())
}

#[derive(Debug, StructOpt)]
struct CaseConfig {
    #[structopt(default_value = "cases", long = "outdir")]
    outdir: PathBuf,
    num_types: u32,
    num_calls: u32,
}

fn gen_one_case(config: CaseConfig) -> Result<()> {
    assert!(config.num_types > 0);
    assert!(config.num_calls > 0);

    println!("generating case: {} types / {} calls", config.num_types, config.num_calls);

    let (static_path, dynamic_path) = gen_src_paths(&config);

    gen_static(&config, &static_path)?;
    gen_dynamic(&config, &dynamic_path)?;

    Ok(())
}

fn gen_all_cases(config: CaseConfig, step: u32) -> Result<()> {
    assert!(step > 0);
    
    for type_num in (1..=config.num_types).step_by(step as usize) {
        let config = CaseConfig {
            outdir: config.outdir.clone(),
            num_types: type_num,
            num_calls: config.num_calls,
        };
        gen_one_case(config)?;
    }

    Ok(())
}

fn compile_one_case(config: CaseConfig) -> Result<()> {
    assert!(config.num_types > 0);
    assert!(config.num_calls > 0);

    println!("compiling case: {} types / {} calls", config.num_types, config.num_calls);

    let (static_src_path, dynamic_src_path) = gen_src_paths(&config);
    let (static_bin_path, dynamic_bin_path) = gen_bin_paths(&config);

    let static_time = run_rustc(&static_src_path, &static_bin_path)?;
    let dynamic_time = run_rustc(&dynamic_src_path, &dynamic_bin_path)?;

    println!("static: {:?}", static_time);
    println!("dynamic: {:?}", dynamic_time);

    Ok(())
}

fn compile_all_cases(config: CaseConfig, step: u32) -> Result<()> {
    assert!(step > 0);
    
    for type_num in (1..=config.num_types).step_by(step as usize) {
        let config = CaseConfig {
            outdir: config.outdir.clone(),
            num_types: type_num,
            num_calls: config.num_calls,
        };
        compile_one_case(config)?;
    }

    Ok(())
}

fn run_one_case(config: CaseConfig) -> Result<()> {
    assert!(config.num_types > 0);
    assert!(config.num_calls > 0);

    println!("running case: {} types / {} calls", config.num_types, config.num_calls);

    let (static_bin_path, dynamic_bin_path) = gen_bin_paths(&config);
    let static_time = run_case(&static_bin_path)?;
    let dynamic_time = run_case(&dynamic_bin_path)?;

    println!("static: {:?}", static_time);
    println!("dynamic: {:?}", dynamic_time);

    Ok(())
}

fn run_all_cases(config: CaseConfig, step: u32) -> Result<()> {
    assert!(step > 0);
    
    for type_num in (1..=config.num_types).step_by(step as usize) {
        let config = CaseConfig {
            outdir: config.outdir.clone(),
            num_types: type_num,
            num_calls: config.num_calls,
        };
        run_one_case(config)?;
    }

    Ok(())
}

fn gen_src_paths(config: &CaseConfig) -> (PathBuf, PathBuf) {
    let mut static_path = config.outdir.clone();
    static_path.push(format!("static-{:04}-{:04}.rs", config.num_types, config.num_calls));
    let mut dynamic_path = config.outdir.clone();
    dynamic_path.push(format!("dynamic-{:04}-{:04}.rs", config.num_types, config.num_calls));
    (static_path, dynamic_path)
}

fn gen_bin_paths(config: &CaseConfig) -> (PathBuf, PathBuf) {
    let mut static_path = config.outdir.clone();
    static_path.push(format!("static-{:04}-{:04}.bin", config.num_types, config.num_calls));
    let mut dynamic_path = config.outdir.clone();
    dynamic_path.push(format!("dynamic-{:04}-{:04}.bin", config.num_types, config.num_calls));
    (static_path, dynamic_path)
}

static HEADER: &'static str = "
#![feature(test)]
extern crate test;

use test::black_box;

trait Io { fn do_io(&self); }
";

static FN_STATIC: &'static str = "
fn do_io<T: Io>(v: &T) {
    v.do_io();
}
";

static FN_DYNAMIC: &'static str = "
fn do_io(v: &dyn Io) {
    v.do_io();
}
";

macro_rules! type_template{ () => { "
struct T{num}({types});
impl Io for T{num} {{ fn do_io(&self) {{ black_box(self); }} }}
"
}}

fn gen_static(config: &CaseConfig, path: &Path) -> Result<()> {
    gen_case(config, path, FN_STATIC)
}

fn gen_dynamic(config: &CaseConfig, path: &Path) -> Result<()> {
    gen_case(config, path, FN_DYNAMIC)
}

const TEST_LOOPS: usize = 100_000;

fn gen_case(config: &CaseConfig, path: &Path, fn_def: &str) -> Result<()> {
    assert!(path.extension().expect("") == "rs");
    let dir = path.parent().expect("directory");
    fs::create_dir_all(&dir)?;
    let mut file = File::create(path)?;

    writeln!(file, "// types = {}, calls = {}",
             config.num_types, config.num_calls)?;
    writeln!(file)?;
    writeln!(file, "{}", HEADER)?;
    writeln!(file, "{}", fn_def)?;

    for num in 0..config.num_types {
        let types = gen_type(num, config.num_types);
        writeln!(file, type_template!(),
                 num = num, types = types)?;
    }

    writeln!(file)?;
    writeln!(file, "fn main() {{")?;

    for type_num in 0..config.num_types {
        writeln!(file, "    let v{num} = &T{num}({ctor});",
                 num = type_num, ctor = gen_ctor(type_num, config.num_types))?;
    }
    writeln!(file)?;

    writeln!(file, "    for _ in 0..{} {{", TEST_LOOPS)?;

    for type_num in 0..config.num_types {
        for _call_num in 0..config.num_calls {
            writeln!(file, "        do_io(v{num});",
                     num = type_num)?;
        }
        writeln!(file)?;
    }

    writeln!(file, "    }}")?;
    writeln!(file, "}}")?;

    file.flush()?;
    drop(file);

    Ok(())
}

fn gen_type(num: u32, num_types: u32) -> String {
    let mut buf = String::new();
    buf.push_str("(");
    buf.push_str(&format!("[u8; {}], ", num));
    buf.push_str("u16,");
    buf.push_str(&format!("[u8; {}], ", num_types - num));
    buf.push_str(")");
    buf
}

fn gen_ctor(num: u32, num_types: u32) -> String {
    let mut buf = String::new();
    buf.push_str("(");
    buf.push_str(&format!("[0_u8; {}], ", num));
    buf.push_str("0_u16,");
    buf.push_str(&format!("[0_u8; {}], ", num_types - num));
    buf.push_str(")");
    buf
}

fn run_rustc(src: &Path, bin: &Path) -> Result<Duration> {
    let start = Instant::now();

    let status = Command::new("rustc")
        .arg(src)
        .arg("-o")
        .arg(bin)
        .status()?;

    if !status.success() {
        bail!("rustc failed");
    }

    let end = Instant::now();

    Ok(end - start)
}

fn run_case(bin: &Path) -> Result<Duration> {
    let start = Instant::now();

    let status = Command::new(bin)
        .status()?;

    if !status.success() {
        bail!("running case failed");
    }

    let end = Instant::now();

    Ok(end - start)
}
