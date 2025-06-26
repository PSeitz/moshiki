use std::{collections::HashSet, io::BufRead};

use moshiki::patterns::preliminary_index;

fn main() {
    // First param is the NDJSON
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <ndjson_file>", args[0]);
        std::process::exit(1);
    }
    let ndjson_file = &args[1];
    println!("Reading NDJSON file: {}", ndjson_file);
    let file_size = std::fs::metadata(ndjson_file)
        .expect("Failed to get file metadata")
        .len();
    let start_time = std::time::Instant::now();
    let file = std::fs::File::open(ndjson_file).expect("Failed to open NDJSON file");
    let reader = std::io::BufReader::new(file);
    let lines = reader
        .lines()
        .map(|line| line.expect("Failed to read line"));
    let preliminary_index = preliminary_index(lines);

    let fingerprints: HashSet<u64> = preliminary_index
        .preliminary_docs
        .iter()
        .map(|doc| doc.fingerprint)
        .collect();
    println!("Num Fingerprints: {}", fingerprints.len());
    println!(
        "Throughput: {:.2} MB/s",
        (file_size as f64 / 1024.0 / 1024.0) / start_time.elapsed().as_secs_f64()
    );
}
