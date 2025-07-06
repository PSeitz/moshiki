use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use binggan::{BenchRunner, INSTRUMENTED_SYSTEM, PeakMemAlloc, black_box, plugins::*};
use moshiki::tokenizer::Tokenizer;

#[global_allocator]
pub static GLOBAL: &PeakMemAlloc<std::alloc::System> = &INSTRUMENTED_SYSTEM;

fn test_tokenizer(lines: impl Iterator<Item = String>) -> u32 {
    lines.map(|line| Tokenizer::new(&line).count() as u32).sum()
}

pub struct Dataset {
    pub name: String,
    pub path: PathBuf,
}

impl Dataset {
    /// Construct a Dataset that will read from `path` at runtime.
    pub fn from_file(name: impl Into<String>, path: impl AsRef<Path>) -> Self {
        Dataset {
            name: name.into(),
            path: path.as_ref().to_path_buf(),
        }
    }

    /// File size in bytes.
    pub fn size(&self) -> u64 {
        self.path.metadata().unwrap().len()
    }

    /// Open the file and return an iterator over its lines.
    ///
    /// Each item is `Ok(line)` or `Err(err)` if reading failed partway through.
    pub fn lines(&self) -> impl Iterator<Item = String> {
        let file = File::open(&self.path).unwrap();
        let reader = BufReader::new(file);
        reader.lines().map(|line| line.unwrap()) // Handle errors by returning empty string
    }
}

/// Build a few named Datasets without doing any I/O yet.
pub fn get_test_data() -> Vec<Dataset> {
    let base = Path::new("./bench_data");
    let specs = [
        ("hdfs-logs", base.join("hdfs-logs")),
        ("windows", base.join("windows")),
        ("android", base.join("android")),
    ];

    specs
        .into_iter()
        .map(|(name, path)| Dataset::from_file(name, path))
        .collect()
}

fn bench_tokenizer() {
    let inputs: Vec<Dataset> = get_test_data();
    let mut runner: BenchRunner = BenchRunner::new();

    runner
        .add_plugin(CacheTrasher::default())
        .add_plugin(PeakMemAllocPlugin::new(GLOBAL));

    for data in inputs.iter() {
        let mut group = runner.new_group();
        group.set_name(&data.name);
        let input_size = data.size();
        group.set_input_size(input_size as usize);
        group.register_with_input("tokenizer", data, move |data| {
            let num_tokens = black_box(test_tokenizer(data.lines()));
            num_tokens as u64
        });
        group.run();
    }
}

fn main() {
    bench_tokenizer();
}
