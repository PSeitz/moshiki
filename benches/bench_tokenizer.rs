use binggan::{black_box, plugins::*, BenchRunner, PeakMemAlloc, INSTRUMENTED_SYSTEM};
use moshiki::tokenizer::Tokenizer;

#[global_allocator]
pub static GLOBAL: &PeakMemAlloc<std::alloc::System> = &INSTRUMENTED_SYSTEM;

fn test_tokenizer(lines: &Vec<&str>) -> u32 {
    lines
        .iter()
        .map(|line| Tokenizer::new(line).count() as u32)
        .sum()
}

fn run_bench() {
    let log_lines = include_str!("../hdfs-logs");
    let inputs: Vec<(&str, Vec<&str>)> = vec![("hdfs-logs", log_lines.lines().collect())];
    let mut runner: BenchRunner = BenchRunner::new();

    runner
        .add_plugin(CacheTrasher::default())
        .add_plugin(PeakMemAllocPlugin::new(GLOBAL));

    for (input_name, data) in inputs.iter() {
        let mut group = runner.new_group();
        group.set_name(input_name);
        let input_size = data.iter().map(|line| line.len()).sum::<usize>();
        group.set_input_size(input_size);
        group.register_with_input("vec", data, move |data| {
            let num_tokens = black_box(test_tokenizer(data));
            num_tokens as u64
        });
        group.run();
    }
}

fn main() {
    run_bench();
}
