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
        num_fns: u32,
        num_calls: u32,
        #[structopt(long = "inline")]
        inline: bool,
    },
    CompileOneCase {
        num_types: u32,
        num_fns: u32,
        num_calls: u32,
    },
    RunOneCase {
        num_types: u32,
        num_fns: u32,
        num_calls: u32,
    },
    GenAllCases {
        num_types: u32,
        num_fns: u32,
        num_calls: u32,
        step_types: u32,
        step_fns: u32,
        step_calls: u32,
        #[structopt(long)]
        inline: bool,
    },
    CompileAllCases {
        num_types: u32,
        num_fns: u32,
        num_calls: u32,
        step_types: u32,
        step_fns: u32,
        step_calls: u32,
    },
    RunAllCases {
        num_types: u32,
        num_fns: u32,
        num_calls: u32,
        step_types: u32,
        step_fns: u32,
        step_calls: u32,
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
        Cmd::GenOneCase { num_types, num_fns, num_calls,
                          inline } => {
            let config = CaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_fns, num_calls
            };
            gen_one_case(config, inline)?;
        }
        Cmd::CompileOneCase { num_types, num_fns, num_calls } => {
            let config = CaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_fns, num_calls
            };
            compile_one_case(config)?;
        }
        Cmd::RunOneCase { num_types, num_fns, num_calls } => {
            let config = CaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_fns, num_calls
            };
            run_one_case(config)?;
        }
        Cmd::GenAllCases { num_types, num_fns, num_calls,
                           step_types, step_fns, step_calls,
                           inline } => {
            let config = MultiCaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_fns, num_calls,
                step_types, step_fns, step_calls,
            };
            gen_all_cases(config, inline)?;
        }
        Cmd::CompileAllCases { num_types, num_fns, num_calls,
                               step_types, step_fns, step_calls, } => {
            let config = MultiCaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_fns, num_calls,
                step_types, step_fns, step_calls,
            };
            compile_all_cases(config)?;
        }
        Cmd::RunAllCases { num_types, num_fns, num_calls,
                           step_types, step_fns, step_calls, } => {
            let config = MultiCaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_fns, num_calls,
                step_types, step_fns, step_calls,
            };
            run_all_cases(config)?;
        }
    }

    Ok(())
}

struct CaseConfig {
    outdir: PathBuf,
    num_types: u32,
    num_fns: u32,
    num_calls: u32,
}

struct MultiCaseConfig {
    outdir: PathBuf,
    num_types: u32,
    num_fns: u32,
    num_calls: u32,
    step_types: u32,
    step_fns: u32,
    step_calls: u32,
}

fn verify_case(config: &CaseConfig) {
    assert!(config.num_types > 0);
    assert!(config.num_fns > 0);
    assert!(config.num_calls > 0);
}

fn prereport(action: &str, config: &CaseConfig) {
    println!("{} case: {} types / {} fns / {} calls",
             action,
             config.num_types,
             config.num_fns,
             config.num_calls);
}

fn gen_one_case(config: CaseConfig, inline: bool) -> Result<()> {
    verify_case(&config);
    prereport("generating", &config);

    let (static_path, dynamic_path) = gen_src_paths(&config);

    gen_static(&config, &static_path, inline)?;
    gen_dynamic(&config, &dynamic_path, inline)?;

    Ok(())
}

fn compile_one_case(config: CaseConfig) -> Result<()> {
    verify_case(&config);
    prereport("compiling", &config);

    let (static_src_path, dynamic_src_path) = gen_src_paths(&config);
    let (static_bin_path, dynamic_bin_path) = gen_bin_paths(&config);

    let static_time = run_rustc(&static_src_path, &static_bin_path)?;
    let dynamic_time = run_rustc(&dynamic_src_path, &dynamic_bin_path)?;

    println!("static-compile-time : {:?}", static_time);
    println!("dynamic-compile-time: {:?}", dynamic_time);

    let static_size = fs::metadata(static_bin_path)?.len();
    let dynamic_size = fs::metadata(dynamic_bin_path)?.len();

    println!("static-bin-size     : {}", static_size);
    println!("dynamic-bin-size    : {}", dynamic_size);

    Ok(())
}

fn run_one_case(config: CaseConfig) -> Result<()> {
    verify_case(&config);
    prereport("running", &config);

    let (static_bin_path, dynamic_bin_path) = gen_bin_paths(&config);
    let static_time = run_case(&static_bin_path)?;
    let dynamic_time = run_case(&dynamic_bin_path)?;

    println!("static-run-time : {:?}", static_time);
    println!("dynamic-run-time: {:?}", dynamic_time);

    Ok(())
}

fn ranges(config: &MultiCaseConfig) ->
    (impl Iterator<Item = u32> + Clone,
     impl Iterator<Item = u32> + Clone,
     impl Iterator<Item = u32> + Clone)
{
    let type_range = if config.step_types > 0 {
        (1..=config.num_types + 1).step_by(config.step_types as usize)
    } else {
        (config.num_types..=config.num_types).step_by(1)
    };
    let fn_range = if config.step_fns > 0 {
        (1..=config.num_fns + 1).step_by(config.step_fns as usize)
    } else {
        (config.num_fns..=config.num_fns).step_by(1)
    };
    let call_range = if config.step_calls > 0 {
        (1..=config.num_calls + 1).step_by(config.step_calls as usize)
    } else {
        (config.num_calls..=config.num_calls).step_by(1)
    };

    (type_range, fn_range, call_range)
}

fn run_all_for(config: MultiCaseConfig, test: impl Fn(CaseConfig) -> Result<()>) -> Result<()> {
    let (type_range, fn_range, call_range) = ranges(&config);
    
    for type_num in type_range {
        for fn_num in fn_range.clone() {
            for call_num in call_range.clone() {
                let config = CaseConfig {
                    outdir: config.outdir.clone(),
                    num_types: type_num,
                    num_fns: fn_num,
                    num_calls: call_num,
                };
                test(config)?;
            }
        }
    }

    Ok(())
}

fn gen_all_cases(config: MultiCaseConfig, inline: bool) -> Result<()> {
    run_all_for(config, |c| gen_one_case(c, inline))
}

fn compile_all_cases(config: MultiCaseConfig) -> Result<()> {
    run_all_for(config, &compile_one_case)
}

fn run_all_cases(config: MultiCaseConfig) -> Result<()> {
    run_all_for(config, &run_one_case)
}

fn gen_src_paths(config: &CaseConfig) -> (PathBuf, PathBuf) {
    let mut static_path = config.outdir.clone();
    static_path.push(
        format!("static-{:04}-{:04}-{:04}.rs",
                config.num_types, config.num_fns, config.num_calls));
    let mut dynamic_path = config.outdir.clone();
    dynamic_path.push(
        format!("dynamic-{:04}-{:04}-{:04}.rs",
                config.num_types, config.num_fns, config.num_calls));
    (static_path, dynamic_path)
}

fn gen_bin_paths(config: &CaseConfig) -> (PathBuf, PathBuf) {
    let mut static_path = config.outdir.clone();
    static_path.push(
        format!("static-{:04}-{:04}-{:04}.bin",
                config.num_types, config.num_fns, config.num_calls));
    let mut dynamic_path = config.outdir.clone();
    dynamic_path.push(
        format!("dynamic-{:04}-{:04}-{:04}.bin",
                config.num_types, config.num_fns, config.num_calls));
    (static_path, dynamic_path)
}

static HEADER: &'static str = "
#![feature(test)]
extern crate test;

use test::black_box;

trait Io { fn do_io_m(&self); }
";

macro_rules! type_template { () => { "
struct T{num}({types});
impl Io for T{num} {{ {inlining} fn do_io_m(&self) {{ black_box(self); }} }}
"
}}

macro_rules! fn_static_template { () => { "
{inlining}
fn do_io_f{num}<T: Io>(v: &T) {{
    v.do_io_m();
    black_box(&{num});
}}
"
}}

macro_rules! fn_dynamic_template { () => { "
{inlining}
fn do_io_f{num}(v: &dyn Io) {{
    v.do_io_m();
    black_box(&{num});
}}
"
}}

fn gen_static(config: &CaseConfig, path: &Path, inline: bool) -> Result<()> {
    gen_case(config, path, write_fn_static, inline)
}

fn gen_dynamic(config: &CaseConfig, path: &Path, inline: bool) -> Result<()> {
    gen_case(config, path, write_fn_dynamic, inline)
}

const TEST_LOOPS: usize = 100_000;

type WriteFn = fn(f: &mut dyn Write, num: u32, inline_str: &str) -> Result<()>;

fn write_fn_static(f: &mut dyn Write, num: u32, inline_str: &str) -> Result<()> {
    Ok(writeln!(f, fn_static_template!(), num = num, inlining = inline_str)?)
}

fn write_fn_dynamic(f: &mut dyn Write, num: u32, inline_str: &str) -> Result<()> {
    Ok(writeln!(f, fn_dynamic_template!(), num = num, inlining = inline_str)?)
}

fn gen_case(config: &CaseConfig, path: &Path,
            write_fn: WriteFn, inline: bool) -> Result<()> {
    assert!(path.extension().expect("") == "rs");
    let dir = path.parent().expect("directory");
    fs::create_dir_all(&dir)?;
    let mut file = File::create(path)?;

    writeln!(file, "// types = {}, calls = {}",
             config.num_types, config.num_calls)?;
    writeln!(file)?;
    writeln!(file, "{}", HEADER)?;

    let inline_str = if inline {
        ""
    } else {
        "#[inline(never)]"
    };

    for type_num in 0..config.num_types {
        let types = gen_type(type_num, config.num_types);
        writeln!(file, type_template!(),
                 num = type_num, types = types,
                 inlining = inline_str)?;
    }

    for fn_num in 0..config.num_fns {
        write_fn(&mut file, fn_num, inline_str)?;
    }

    writeln!(file)?;
    writeln!(file, "fn main() {{")?;

    for type_num in 0..config.num_types {
        writeln!(file, "    static V{num}: &T{num} = &T{num}({ctor});",
                 num = type_num, ctor = gen_ctor(type_num, config.num_types))?;
    }
    writeln!(file)?;

    writeln!(file, "    for _ in 0..{} {{", TEST_LOOPS)?;

    for fn_num in 0..config.num_fns {
        for type_num in 0..config.num_types {
            for _call_num in 0..config.num_calls {
                writeln!(file, "        do_io_f{fn_num}(V{type_num});",
                         fn_num = fn_num,
                         type_num = type_num)?;
            }
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
    buf.push_str("u16, ");
    buf.push_str(&format!("[u8; {}], ", num_types - num));
    buf.push_str(")");
    buf
}

fn gen_ctor(num: u32, num_types: u32) -> String {
    let mut buf = String::new();
    buf.push_str("(");
    buf.push_str(&format!("[0_u8; {}], ", num));
    buf.push_str("0_u16, ");
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
        .arg("-Copt-level=3")
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
