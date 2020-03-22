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
        #[structopt(long)]
        no_inline: bool,
        #[structopt(long)]
        no_dedup: bool,
        #[structopt(long)]
        predictable: bool,
    },
    CompileOneCase {
        num_types: u32,
        num_fns: u32,
        #[structopt(long)]
        asm: bool,
        #[structopt(long, default_value = "0")]
        opt_level: u32,
    },
    RunOneCase {
        num_types: u32,
        num_fns: u32,
    },
    GenAllCases {
        num_types: u32,
        num_fns: u32,
        step_types: u32,
        step_fns: u32,
        #[structopt(long)]
        no_inline: bool,
        #[structopt(long)]
        no_dedup: bool,
        #[structopt(long)]
        predictable: bool,
    },
    CompileAllCases {
        num_types: u32,
        num_fns: u32,
        step_types: u32,
        step_fns: u32,
        #[structopt(long)]
        asm: bool,
        #[structopt(long, default_value = "0")]
        opt_level: u32,
    },
    RunAllCases {
        num_types: u32,
        num_fns: u32,
        step_types: u32,
        step_fns: u32,
    },
}

#[derive(Debug, StructOpt)]
struct GlobalOptions {
    #[structopt(default_value = "cases", long)]
    outdir: PathBuf,
}

fn main() -> Result<()> {
    let options = Options::from_args();

    match options.cmd {
        Cmd::GenOneCase { num_types, num_fns,
                          no_inline, no_dedup, predictable } => {
            let config = CaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_fns,
            };
            let opts = GenOpts {
                no_inline, no_dedup, predictable
            };
            gen_one_case(config, opts)?;
        }
        Cmd::CompileOneCase { num_types, num_fns,
                              asm, opt_level } => {
            let config = CaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_fns,
            };
            let opts = CompileOpts {
                asm, opt_level
            };
            compile_one_case(config, opts)?;
        }
        Cmd::RunOneCase { num_types, num_fns } => {
            let config = CaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_fns,
            };
            run_one_case(config)?;
        }
        Cmd::GenAllCases { num_types, num_fns,
                           step_types, step_fns,
                           no_inline, no_dedup, predictable } => {
            let config = MultiCaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_fns,
                step_types, step_fns,
            };
            let opts = GenOpts {
                no_inline, no_dedup, predictable
            };
            gen_all_cases(config, opts)?;
        }
        Cmd::CompileAllCases { num_types, num_fns,
                               step_types, step_fns,
                               asm, opt_level } => {
            let config = MultiCaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_fns,
                step_types, step_fns,
            };
            let opts = CompileOpts {
                asm, opt_level
            };
            compile_all_cases(config, opts)?;
        }
        Cmd::RunAllCases { num_types, num_fns,
                           step_types, step_fns, } => {
            let config = MultiCaseConfig {
                outdir: options.global.outdir.clone(),
                num_types, num_fns,
                step_types, step_fns,
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
}

struct MultiCaseConfig {
    outdir: PathBuf,
    num_types: u32,
    num_fns: u32,
    step_types: u32,
    step_fns: u32,
}

#[derive(Clone)]
struct CompileOpts {
    asm: bool,
    opt_level: u32,
}

#[derive(Clone)]
struct GenOpts {
    no_inline: bool,
    no_dedup: bool,
    predictable: bool,
}

fn prereport(action: &str, config: &CaseConfig) {
    println!("{} case: {} types / {} fns",
             action,
             config.num_types,
             config.num_fns);
}

fn gen_one_case(config: CaseConfig, opts: GenOpts) -> Result<()> {
    prereport("generating", &config);

    let (static_path, dynamic_path) = gen_src_paths(&config);

    gen_static(&config, &static_path, opts.clone())?;
    gen_dynamic(&config, &dynamic_path, opts)?;

    Ok(())
}

fn compile_one_case(config: CaseConfig, opts: CompileOpts) -> Result<()> {
    prereport("compiling", &config);

    let (static_src_path, dynamic_src_path) = gen_src_paths(&config);
    let (static_bin_path, dynamic_bin_path) = gen_bin_paths(&config);

    let static_time = run_rustc_bin(&static_src_path, &static_bin_path, &opts)?;
    let dynamic_time = run_rustc_bin(&dynamic_src_path, &dynamic_bin_path, &opts)?;

    println!("static-compile-time  : {}", static_time.as_millis());
    println!("dynamic-compile-time : {}", dynamic_time.as_millis());

    let static_size = fs::metadata(&static_bin_path)?.len();
    let dynamic_size = fs::metadata(&dynamic_bin_path)?.len();

    println!("static-bin-size      : {}", static_size);
    println!("dynamic-bin-size     : {}", dynamic_size);

    if opts.asm {
        let (static_asm_path, dynamic_asm_path) = gen_asm_paths(&config);

        run_rustc_asm(&static_src_path, &static_asm_path, &opts)?;
        run_rustc_asm(&dynamic_src_path, &dynamic_asm_path, &opts)?;
    }

    let (static_method_count, static_fn_count)
        = count_symbols(&static_bin_path)?;
    let (dynamic_method_count, dynamic_fn_count)
        = count_symbols(&dynamic_bin_path)?;

    println!("static-method-count  : {}", static_method_count);
    println!("static-fn-count      : {}", static_fn_count);
    println!("dynamic-method-count : {}", dynamic_method_count);
    println!("dynamic-fn-count     : {}", dynamic_fn_count);

    Ok(())
}

fn run_one_case(config: CaseConfig) -> Result<()> {
    prereport("running", &config);

    let (static_bin_path, dynamic_bin_path) = gen_bin_paths(&config);
    let static_time = run_case(&static_bin_path)?;
    let dynamic_time = run_case(&dynamic_bin_path)?;

    println!("static-run-time : {}", static_time.as_millis());
    println!("dynamic-run-time: {}", dynamic_time.as_millis());

    Ok(())
}

fn ranges(config: &MultiCaseConfig) ->
    (impl Iterator<Item = u32> + Clone,
     impl Iterator<Item = u32> + Clone)
{
    let type_range = if config.step_types > 0 {
        (0..=config.num_types).step_by(config.step_types as usize)
    } else {
        (config.num_types..=config.num_types).step_by(1)
    };
    let fn_range = if config.step_fns > 0 {
        (0..=config.num_fns).step_by(config.step_fns as usize)
    } else {
        (config.num_fns..=config.num_fns).step_by(1)
    };

    (type_range, fn_range)
}

fn run_all_for(config: MultiCaseConfig, test: impl Fn(CaseConfig) -> Result<()>) -> Result<()> {
    let (type_range, fn_range) = ranges(&config);
    
    for type_num in type_range {
        for fn_num in fn_range.clone() {
            let config = CaseConfig {
                outdir: config.outdir.clone(),
                num_types: type_num,
                num_fns: fn_num,
            };
            test(config)?;
        }
    }

    Ok(())
}

fn gen_all_cases(config: MultiCaseConfig, opts: GenOpts) -> Result<()> {
    run_all_for(config, |c| gen_one_case(c, opts.clone()))
}

fn compile_all_cases(config: MultiCaseConfig, opts: CompileOpts) -> Result<()> {
    run_all_for(config, |c| compile_one_case(c, opts.clone()))
}

fn run_all_cases(config: MultiCaseConfig) -> Result<()> {
    run_all_for(config, &run_one_case)
}

fn gen_src_paths(config: &CaseConfig) -> (PathBuf, PathBuf) {
    gen_paths(config, "rs")
}

fn gen_bin_paths(config: &CaseConfig) -> (PathBuf, PathBuf) {
    gen_paths(config, "bin")
}

fn gen_asm_paths(config: &CaseConfig) -> (PathBuf, PathBuf) {
    gen_paths(config, "S")
}

fn gen_paths(config: &CaseConfig, ext: &str) -> (PathBuf, PathBuf) {
    let mut static_path = config.outdir.clone();
    static_path.push(
        format!("static-{:04}-{:04}.{}",
                config.num_types, config.num_fns,
                ext));
    let mut dynamic_path = config.outdir.clone();
    dynamic_path.push(
        format!("dynamic-{:04}-{:04}.{}",
                config.num_types, config.num_fns,
                ext));
    (static_path, dynamic_path)
}


static HEADER: &'static str = "
#![feature(test)]

use std::hint::black_box;

trait Io { fn do_io_m(&self); }
";

macro_rules! type_template { () => { "
struct T{num}({types});
impl Io for T{num} {{
    {inlining}
    fn do_io_m(&self) {{
        black_box(self);
        if {no_dedup} {{
            black_box(&{num});
        }}
    }}
}}
"
}}

macro_rules! fn_static_template { () => { "
{inlining}
fn do_io_f{num}<T: Io>(v: &T) {{
    v.do_io_m();
    if {no_dedup} {{
        black_box(&{num});
    }}
}}
"
}}

macro_rules! fn_dynamic_template { () => { "
{inlining}
fn do_io_f{num}(v: &dyn Io) {{
    v.do_io_m();
    if {no_dedup} {{
        black_box(&{num});
    }}
}}
"
}}

fn gen_static(config: &CaseConfig, path: &Path, opts: GenOpts) -> Result<()> {
    gen_case(config, path, write_fn_static, opts)
}

fn gen_dynamic(config: &CaseConfig, path: &Path, opts: GenOpts) -> Result<()> {
    gen_case(config, path, write_fn_dynamic, opts)
}

const TEST_LOOPS: usize = 100_000;

type WriteFn = fn(f: &mut dyn Write, num: u32, opts: &GenOpts) -> Result<()>;

fn write_fn_static(f: &mut dyn Write, num: u32, opts: &GenOpts) -> Result<()> {
    Ok(writeln!(f, fn_static_template!(),
                num = num, inlining = inline_str(opts),
                no_dedup = opts.no_dedup)?)
}

fn write_fn_dynamic(f: &mut dyn Write, num: u32, opts: &GenOpts) -> Result<()> {
    Ok(writeln!(f, fn_dynamic_template!(),
                num = num, inlining = inline_str(opts),
                no_dedup = opts.no_dedup)?)
}

fn inline_str(opts: &GenOpts) -> &'static str {
    if opts.no_inline {
        "#[inline(never)]"
    } else {
        ""
    }
}

fn gen_case(config: &CaseConfig, path: &Path,
            write_fn: WriteFn, opts: GenOpts) -> Result<()> {
    assert!(path.extension().expect("") == "rs");
    let dir = path.parent().expect("directory");
    fs::create_dir_all(&dir)?;
    let mut file = File::create(path)?;

    writeln!(file, "// types = {}, fns = {}",
             config.num_types, config.num_fns)?;
    writeln!(file)?;

    if config.num_types == 0 || config.num_fns == 0 {
        writeln!(file, "#![allow(unused)]")?;
    }

    writeln!(file, "{}", HEADER)?;

    for type_num in 0..config.num_types {
        let types = gen_type(type_num, config.num_types);
        writeln!(file, type_template!(),
                 num = type_num, types = types,
                 inlining = inline_str(&opts),
                 no_dedup = opts.no_dedup)?;
    }

    for fn_num in 0..config.num_fns {
        write_fn(&mut file, fn_num, &opts)?;
    }

    writeln!(file)?;
    writeln!(file, "fn main() {{")?;

    for type_num in 0..config.num_types {
        writeln!(file, "    static V{num}: &T{num} = &T{num}({ctor});",
                 num = type_num, ctor = gen_ctor(type_num, config.num_types))?;
    }
    writeln!(file)?;

    writeln!(file, "    for _ in 0..{} {{", TEST_LOOPS)?;

    if !opts.predictable {
        for fn_num in 0..config.num_fns {
            for type_num in 0..config.num_types {
                writeln!(file, "        do_io_f{fn_num}(V{type_num});",
                         fn_num = fn_num,
                         type_num = type_num)?;
            }
            writeln!(file)?;
        }
    } else {
        for type_num in 0..config.num_types {
            for fn_num in 0..config.num_fns {
                writeln!(file, "        do_io_f{fn_num}(V{type_num});",
                         fn_num = fn_num,
                         type_num = type_num)?;
            }
            writeln!(file)?;
        }
    }

    writeln!(file, "    }}")?;
    writeln!(file, "}}")?;

    file.flush()?;
    drop(file);

    Ok(())
}

fn gen_type(num: u32, num_types: u32) -> String {
    "u8".to_string()
}

fn gen_ctor(num: u32, num_types: u32) -> String {
    "0_u8".to_string()
}

fn run_rustc_bin(src: &Path, out: &Path, opts: &CompileOpts) -> Result<Duration> {
    run_rustc(src, out, "link", opts)
}

fn run_rustc_asm(src: &Path, out: &Path, opts: &CompileOpts) -> Result<Duration> {
    run_rustc(src, out, "asm", opts)
}

fn run_rustc(src: &Path, out: &Path, emit: &str, opts: &CompileOpts) -> Result<Duration> {
    let start = Instant::now();

    let status = Command::new("rustc")
        .arg(src)
        .arg("--emit")
        .arg(emit)
        .arg("-o")
        .arg(out)
        .arg(format!("-Copt-level={}", opts.opt_level))
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

fn count_symbols(bin: &Path) -> Result<(usize, usize)> {
    let output = Command::new("nm")
        .arg(bin)
        .output()?;

    if !output.status.success() {
        bail!("running nm failed");
    }

    let out_str = String::from_utf8_lossy(&output.stdout);
    let lines = out_str.lines();
    let method_count = lines.clone().filter(|s| s.contains("do_io_m")).count();
    let fn_count = lines.filter(|s| s.contains("do_io_f")).count();

    Ok((method_count, fn_count))
}


