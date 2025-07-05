use binggan::{BenchRunner, INSTRUMENTED_SYSTEM, PeakMemAlloc, black_box, plugins::*};
use moshiki::{indexing::preliminary_index, tokenizer::Tokenizer};

#[global_allocator]
pub static GLOBAL: &PeakMemAlloc<std::alloc::System> = &INSTRUMENTED_SYSTEM;

fn test_tokenizer(lines: &Vec<&str>) -> u32 {
    lines
        .iter()
        .map(|line| Tokenizer::new(line).count() as u32)
        .sum()
}

pub struct Dataset {
    pub name: &'static str,
    pub file_content: &'static str,
    pub lines: Vec<&'static str>,
}
impl Dataset {
    pub fn len(&self) -> usize {
        self.file_content.len()
    }
    pub fn is_empty(&self) -> bool {
        self.file_content.is_empty()
    }
}
impl From<(&'static str, &'static str)> for Dataset {
    fn from(data: (&'static str, &'static str)) -> Self {
        Dataset {
            name: data.0,
            file_content: data.1,
            lines: data.1.lines().collect(),
        }
    }
}

pub fn get_test_data() -> Vec<Dataset> {
    vec![
        (
            "hdfs-logs",
            include_str!("../tokenizer_bench_data/hdfs-logs"),
        )
            .into(),
        ("windows", include_str!("../tokenizer_bench_data/windows")).into(),
        ("android", include_str!("../tokenizer_bench_data/android")).into(),
    ]
}

fn bench_tokenizer() {
    let inputs: Vec<Dataset> = get_test_data();
    let mut runner: BenchRunner = BenchRunner::new();

    runner
        .add_plugin(CacheTrasher::default())
        .add_plugin(PeakMemAllocPlugin::new(GLOBAL));

    for data in inputs.iter() {
        let mut group = runner.new_group();
        group.set_name(data.name);
        let input_size = data.lines.iter().map(|line| line.len()).sum::<usize>();
        group.set_input_size(input_size);
        group.register_with_input("tokenizer", data, move |data| {
            let num_tokens = black_box(test_tokenizer(&data.lines));
            num_tokens as u64
        });
        group.run();
    }
}

fn bench_mini_index() {
    let inputs: Vec<Dataset> = get_test_data();
    let mut runner: BenchRunner = BenchRunner::new();

    runner
        .add_plugin(CacheTrasher::default())
        .add_plugin(PeakMemAllocPlugin::new(GLOBAL));

    for dataset in inputs.iter() {
        let mut group = runner.new_group();
        group.set_name(dataset.name);
        let input_size = dataset.len();
        group.set_input_size(input_size);
        group.register_with_input("mini index", dataset, move |dataset| {
            let mini_index = black_box(preliminary_index(
                dataset.lines.iter().map(|s| s.to_string()),
            ));
            mini_index.doc_groups.len() as u64
        });
        group.run();
    }
}

fn main() {
    bench_tokenizer();
    bench_mini_index();
}
