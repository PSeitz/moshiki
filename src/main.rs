use std::cmp::max;
use std::io::BufRead;
use std::path::Path;
use std::{fs, io};

use moshiki::indexing::IndexWriter;

struct Report {
    file_name: String,
    throughput: f64,
    input_size: u64,
    output_size: u64,
}
impl Report {
    pub fn compression_ratio(&self) -> f64 {
        self.output_size as f64 / self.input_size as f64
    }

    fn input_size_mb(&self) -> f64 {
        self.input_size as f64 / 1024.0 / 1024.0
    }

    fn output_size_mb(&self) -> f64 {
        self.output_size as f64 / 1024.0 / 1024.0
    }
}

fn print_reports(reports: &[Report]) {
    // Width of the first column = longest file name or header, whichever is larger.
    let name_width = max(
        "File Name".len(),
        reports.iter().map(|r| r.file_name.len()).max().unwrap_or(0),
    );

    println!(
        "{:<name_width$}  {:>15}  {:>15}  {:>15} {:>15}",
        "File Name",
        "Throughput (MB/s)",
        "Input Size (MB)",
        "Output Size (MB)",
        "Compression Ratio",
        name_width = name_width
    );

    for r in reports {
        println!(
            "{:<name_width$}  {:>15.2}  {:>15.2}  {:>15.2} {:>15.4}",
            r.file_name,
            r.throughput,
            r.input_size_mb(),
            r.output_size_mb(),
            r.compression_ratio(),
            name_width = name_width
        );
    }
}

/// Return the sum of `len()` for every regular file **directly inside** `dir`.
///
/// * Sub-directories, symlinks, sockets, etc. are ignored.
/// * Fails fast on the first I/O error (you can change that if you prefer).
pub fn folder_size<P: AsRef<Path>>(dir: P) -> io::Result<u64> {
    let mut total: u64 = 0;

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let md = entry.metadata()?; // one `stat` call per entry

        if md.is_file() {
            total += md.len(); // logical byte length
        }
    }

    Ok(total)
}

fn generate_report(ndjson_files: &[String], output_folder: &str) -> io::Result<()> {
    let mut reports = Vec::new();

    print!("Indexing file: ");
    for ndjson_file in ndjson_files {
        if Some(ndjson_file) == ndjson_files.last() {
            println!("{ndjson_file} ");
        } else {
            print!("{ndjson_file}, ");
        }
        let start_time = std::time::Instant::now();
        index_file(ndjson_file, output_folder, false)?;

        let file_size = fs::metadata(ndjson_file)
            .expect("Failed to get file metadata")
            .len();
        reports.push(Report {
            file_name: ndjson_file.clone(),
            throughput: (file_size as f64 / 1024.0 / 1024.0) / start_time.elapsed().as_secs_f64(),
            input_size: file_size,
            output_size: folder_size(output_folder)?,
        });
    }
    print_reports(&reports);
    Ok(())
}

fn main() {
    // First param is the NDJSON
    let args: Vec<String> = std::env::args().collect();
    if args.get(1) == Some(&"report".to_string()) {
        let files = args.get(2..).unwrap_or(&[]);
        // If the first param is a directory, read all files in it
        if std::fs::metadata(&files[0])
            .map(|m| m.is_dir())
            .unwrap_or(false)
        {
            let dir = &files[0];
            let files: Vec<String> = fs::read_dir(dir)
                .expect("Failed to read directory")
                .map(|entry| entry.unwrap().path().to_string_lossy().to_string())
                .collect();
            generate_report(&files, "out").unwrap();
            return;
        }
        generate_report(files, "out").unwrap();
        return;
    }
    if args.len() < 3 {
        eprintln!("Usage: {} <ndjson_file> <output_folder>", args[0]);
        std::process::exit(1);
    }
    let ndjson_file = &args[1];
    let output_folder = &args[2];

    index_file(ndjson_file, output_folder, true).unwrap();
}

pub fn index_file(ndjson_file: &str, output_folder: &str, report: bool) -> std::io::Result<()> {
    let file_size = fs::metadata(ndjson_file)
        .expect("Failed to get file metadata")
        .len();
    let start_time = std::time::Instant::now();
    // Delete the output folder if it exists
    if Path::new(output_folder).exists() {
        fs::remove_dir_all(output_folder)?;
    }
    fs::create_dir_all(output_folder)?;

    let file = fs::File::open(ndjson_file)?;
    let reader = std::io::BufReader::new(file);
    let lines = reader
        .lines()
        .map(|line| line.expect("Failed to read line"));

    let writer = IndexWriter::new(output_folder.to_string());
    writer.index(lines, report)?;
    if report {
        println!(
            "{ndjson_file}: Throughput: {:.2} MB/s",
            (file_size as f64 / 1024.0 / 1024.0) / start_time.elapsed().as_secs_f64()
        );
    }
    Ok(())
}
