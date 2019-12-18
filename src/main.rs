#![allow(unused)]

use std::env;
use std::path::{PathBuf, Path};
use std::fs::{self, File};
use std::iter;
use std::io::Write;

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let num_types: usize =
        args.get(1)
        .expect("arg 1 - num-types")
        .parse()
        .expect("arg 1 usize");
    let call_depth: usize =
        args.get(2)
        .expect("arg 2 - call-depth")
        .parse()
        .expect("arg 2 usize");

    assert!(num_types > 0);
    assert!(call_depth > 0);

    let config = Config {
        outpath: PathBuf::from("cases"),
        num_types,
        call_depth,
    };

    generate(config);
}

struct Config {
    outpath: PathBuf,
    num_types: usize,
    call_depth: usize,
}

fn generate(config: Config) {
    let (static_path, dynamic_path) = gen_paths(&config);

    gen_static(&config, &static_path);
    gen_dynamic(&config, &dynamic_path);
}

fn gen_paths(config: &Config) -> (PathBuf, PathBuf) {
    let mut static_path = config.outpath.clone();
    static_path.push(format!("static-{:04}-{:04}.rs", config.num_types, config.call_depth));
    let mut dynamic_path = config.outpath.clone();
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

fn gen_static(config: &Config, path: &Path) {
    assert!(path.extension().expect("") == "rs");
    let dir = path.parent().expect("directory");
    fs::create_dir_all(&dir).expect("create dir");
    let mut file = File::create(path).expect("");
    writeln!(file, "// types = {}, depth = {}",
             config.num_types, config.call_depth).expect("");
    writeln!(file).expect("");
    writeln!(file, "{}", HEADER).expect("");

    for num in 0..config.num_types {
        let types = "u8, ".repeat(num);
        writeln!(file, type_template!(),
                 num = num, types = types);
    }

    file.flush().expect("");
    drop(file);
}

fn gen_dynamic(config: &Config, path: &Path) {
}
