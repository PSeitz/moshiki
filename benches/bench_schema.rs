use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use binggan::{BenchRunner, INSTRUMENTED_SYSTEM, PeakMemAlloc, black_box, plugins::*};
use moshiki::schema::SchemaTree;

#[global_allocator]
pub static GLOBAL: &PeakMemAlloc<std::alloc::System> = &INSTRUMENTED_SYSTEM;

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
    pub fn lines(&self) -> impl Iterator<Item = String> {
        let file = File::open(&self.path).unwrap();
        let reader = BufReader::new(file);
        reader.lines().map(|line| line.unwrap())
    }
}

/// Build named Datasets without doing any I/O yet.
pub fn get_test_data() -> Vec<Dataset> {
    let specs = [("hdfs-logs-json", Path::new("./hdfs-logs.json"))];

    specs
        .into_iter()
        .map(|(name, path)| Dataset::from_file(name, path))
        .collect()
}

fn bench_schema() {
    let inputs: Vec<Dataset> = get_test_data();
    let mut runner: BenchRunner = BenchRunner::new();

    runner
        .add_plugin(CacheTrasher::default())
        .add_plugin(PeakMemAllocPlugin::new(GLOBAL));

    for dataset in inputs.iter() {
        let mut group = runner.new_group();
        group.set_name(&dataset.name);
        group.set_input_size(dataset.size() as usize);
        group.register_with_input("schema", dataset, move |dataset| {
            let mut tree = SchemaTree::new();
            let mut total_leaf_ids = 0u64;
            for line in dataset.lines() {
                let schema_id = tree.ingest_json(&line).unwrap();
                total_leaf_ids += schema_id.leaf_ids().len() as u64;
            }
            black_box(total_leaf_ids)
        });
        group.run();
    }
}

fn main() {
    bench_schema();
}
