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
struct Config {
    #[structopt(default_value = "cases", long = "outdir")]
    outdir: PathBuf,
    num_types: usize,
    call_depth: usize,
}

fn main() -> Result<()> {
    let config = Config::from_args();

    assert!(config.num_types > 0);
    assert!(config.call_depth > 0);

    generate(config)?;

    Ok(())
}

fn generate(config: Config) -> Result<()> {
    let (static_path, dynamic_path) = gen_paths(&config);

    gen_static(&config, &static_path)?;
    gen_dynamic(&config, &dynamic_path)?;

    Ok(())
}

fn gen_paths(config: &Config) -> (PathBuf, PathBuf) {
    let mut static_path = config.outdir.clone();
    static_path.push(format!("static-{:04}-{:04}.rs", config.num_types, config.call_depth));
    let mut dynamic_path = config.outdir.clone();
    dynamic_path.push(format!("dynamic-{:04}-{:04}.rs", config.num_types, config.call_depth));
    (static_path, dynamic_path)
}

static HEADER: &'static str = "
#![feature(test)]
extern crate test;

use test::black_box;

trait Io { fn do_io(&self); }
";

macro_rules! type_template{ () => { "
#[derive(Debug)]
struct T{num}({types});
impl Io for T{num} {{ fn do_io(&self) {{ black_box(self) }} }}
"
}}

fn gen_static(config: &Config, path: &Path) -> Result<()> {
    assert!(path.extension().expect("") == "rs");
    let dir = path.parent().expect("directory");
    fs::create_dir_all(&dir)?;
    let mut file = File::create(path)?;
    writeln!(file, "// types = {}, depth = {}",
             config.num_types, config.call_depth)?;
    writeln!(file)?;
    writeln!(file, "{}", HEADER)?;

    for num in 0..config.num_types {
        let types = "u8, ".repeat(num);
        writeln!(file, type_template!(),
                 num = num, types = types)?;
    }

    file.flush()?;
    drop(file);

    Ok(())
}

fn gen_dynamic(config: &Config, path: &Path) -> Result<()> {
    panic!()
}
