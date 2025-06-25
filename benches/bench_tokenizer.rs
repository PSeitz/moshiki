use binggan::{black_box, plugins::*, BenchRunner, PeakMemAlloc, INSTRUMENTED_SYSTEM};
use moshiki::{patterns::preliminary_index, tokenizer::Tokenizer};

#[global_allocator]
pub static GLOBAL: &PeakMemAlloc<std::alloc::System> = &INSTRUMENTED_SYSTEM;

fn test_tokenizer(lines: &Vec<&str>) -> u32 {
    lines
        .iter()
        .map(|line| Tokenizer::new(line).count() as u32)
        .sum()
}
const LOG_LINES: &str = include_str!("../hdfs-logs");

fn bench_tokenizer() {
    let inputs: Vec<(&str, Vec<&str>)> = vec![("hdfs-logs", LOG_LINES.lines().collect())];
    let mut runner: BenchRunner = BenchRunner::new();

    runner
        .add_plugin(CacheTrasher::default())
        .add_plugin(PeakMemAllocPlugin::new(GLOBAL));

    for (input_name, data) in inputs.iter() {
        let mut group = runner.new_group();
        group.set_name(input_name);
        let input_size = data.iter().map(|line| line.len()).sum::<usize>();
        group.set_input_size(input_size);
        group.register_with_input("tokenizer", data, move |data| {
            let num_tokens = black_box(test_tokenizer(data));
            num_tokens as u64
        });
        group.run();
    }
}

fn bench_mini_index() {
    let inputs: Vec<(&str, &str)> = vec![("hdfs-logs", LOG_LINES)];
    let mut runner: BenchRunner = BenchRunner::new();

    runner
        .add_plugin(CacheTrasher::default())
        .add_plugin(PeakMemAllocPlugin::new(GLOBAL));

    for (input_name, data) in inputs.iter() {
        let mut group = runner.new_group();
        group.set_name(input_name);
        let input_size = data.len();
        group.set_input_size(input_size);
        group.register_with_input("mini index", data, move |data| {
            let mini_index = black_box(preliminary_index(data.lines().map(|s| s.to_string())));
            mini_index.preliminary_docs.len() as u64
        });
        group.run();
    }
}

fn main() {
    //bench_tokenizer();
    bench_mini_index();
}
