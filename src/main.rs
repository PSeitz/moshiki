use std::fs;
use std::io::BufRead;
use std::path::Path;

use moshiki::indexing::IndexWriter;

fn main() {
    // First param is the NDJSON
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <ndjson_file> <output_folder>", args[0]);
        std::process::exit(1);
    }
    let ndjson_file = &args[1];
    let output_folder = &args[2];

    // Delete the output folder if it exists
    if Path::new(output_folder).exists() {
        fs::remove_dir_all(output_folder).expect("Failed to remove existing output folder");
    }
    // Create the output folder if it doesn't exist
    if !Path::new(output_folder).exists() {
        fs::create_dir_all(output_folder).expect("Failed to create output folder");
    }

    println!("Reading NDJSON file: {ndjson_file}");
    let file_size = std::fs::metadata(ndjson_file)
        .expect("Failed to get file metadata")
        .len();
    let start_time = std::time::Instant::now();
    let file = std::fs::File::open(ndjson_file).expect("Failed to open NDJSON file");
    let reader = std::io::BufReader::new(file);
    let lines = reader
        .lines()
        .map(|line| line.expect("Failed to read line"));

    let writer = IndexWriter::new(output_folder.to_string());
    writer.index(lines);

    println!(
        "Throughput: {:.2} MB/s",
        (file_size as f64 / 1024.0 / 1024.0) / start_time.elapsed().as_secs_f64()
    );
}
