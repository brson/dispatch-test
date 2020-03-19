#![allow(unused)]

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;

use structopt::StructOpt;
use std::env;
use std::path::{PathBuf, Path};
use std::fs::{self, File};
use std::iter;
use std::io::Write;
use anyhow::Result;

#[derive(Debug, StructOpt)]
struct CaseConfig {
    #[structopt(default_value = "cases", long = "outdir")]
    outdir: PathBuf,
    num_types: usize,
    num_calls: usize,
}

fn main() -> Result<()> {
    let config = CaseConfig::from_args();

    assert!(config.num_types > 0);
    assert!(config.num_calls > 0);

    gen_one_case(config)?;

    Ok(())
}

fn gen_one_case(config: CaseConfig) -> Result<()> {
    let (static_path, dynamic_path) = gen_paths(&config);

    gen_static(&config, &static_path)?;
    gen_dynamic(&config, &dynamic_path)?;

    Ok(())
}

fn gen_paths(config: &CaseConfig) -> (PathBuf, PathBuf) {
    let mut static_path = config.outdir.clone();
    static_path.push(format!("static-{:04}-{:04}.rs", config.num_types, config.num_calls));
    let mut dynamic_path = config.outdir.clone();
    dynamic_path.push(format!("dynamic-{:04}-{:04}.rs", config.num_types, config.num_calls));
    (static_path, dynamic_path)
}

static HEADER: &'static str = "
#![feature(test)]
extern crate test;

use test::black_box;

trait Io { fn do_io(&self); }
";

static FN_STATIC: &'static str = "
fn do_io<T: Io>(v: Io) {
    v.do_io();
}
";

static FN_DYNAMIC: &'static str = "
fn do_io(v: &dyn Io) {
    v.do_io();
}
";

macro_rules! type_template{ () => { "
#[derive(Debug, Default)]
struct T{num}({types});
impl Io for T{num} {{ fn do_io(&self) {{ black_box(self) }} }}
"
}}

fn gen_static(config: &CaseConfig, path: &Path) -> Result<()> {
    gen_case(config, path, FN_STATIC)
}

fn gen_dynamic(config: &CaseConfig, path: &Path) -> Result<()> {
    gen_case(config, path, FN_DYNAMIC)
}

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
        writeln!(file, "    let v{num} = &T{num}::default();",
                 num = type_num)?;
        for _call_num in 0..config.num_calls {
            writeln!(file, "    do_io(v{num});",
                     num = type_num)?;
        }
        writeln!(file)?;
    }
    
    writeln!(file, "}}")?;

    file.flush()?;
    drop(file);

    Ok(())
}

fn gen_type(num: usize, num_types: usize) -> String {
    let mut buf = String::new();
    buf.push_str("[");
    for i in 0..num_types {
        if i == num {
            buf.push_str("u8, ");
        } else {
            buf.push_str("u16, ");
        }
    }
    buf.push_str("]");
    buf
}
