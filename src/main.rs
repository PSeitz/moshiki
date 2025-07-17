use std::cmp::max;
use std::fs::File;
use std::io::{BufRead, Write};
use std::path::Path;
use std::{fs, io};

use moshiki::constants::{CATCH_ALL_DICTIONARY_NAME, DICTIONARY_NAME};
use moshiki::index::Index;
use moshiki::indexing::IndexWriter;

use tikv_jemallocator::Jemalloc;

#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

struct Report {
    file_name: String,
    throughput: f64,
    input_size: u64,
    output_size: u64,
    zstd_compressed_size: u64,
    catch_all_dictionary_size: u64,
    dictionary_size: u64,
}
impl Report {
    pub fn compression_ratio(&self) -> f64 {
        1.0 / (self.output_size as f64 / self.input_size as f64)
    }

    fn input_size_mb(&self) -> f64 {
        self.input_size as f64 / 1024.0 / 1024.0
    }
    fn zstd_compressed_size_mb(&self) -> f64 {
        self.zstd_compressed_size as f64 / 1024.0 / 1024.0
    }
    fn output_size_mb(&self) -> f64 {
        self.output_size as f64 / 1024.0 / 1024.0
    }
    fn catch_all_dict_size_mb(&self) -> f64 {
        self.catch_all_dictionary_size as f64 / 1024.0 / 1024.0
    }
    fn dictionary_size_mb(&self) -> f64 {
        self.dictionary_size as f64 / 1024.0 / 1024.0
    }
}

/// A writer that counts the number of bytes written.
struct CountingWriter {
    count: u64,
}
impl CountingWriter {
    fn new() -> Self {
        CountingWriter { count: 0 }
    }
    /// Returns the total number of bytes written so far.
    fn bytes_written(&self) -> u64 {
        self.count
    }
}

impl Write for CountingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let len = buf.len();
        // Just count the bytes, do not store them
        self.count += len as u64;
        Ok(len)
    }

    fn flush(&mut self) -> io::Result<()> {
        // Nothing to flush
        Ok(())
    }
}

/// Returns the size in bytes of the zstd-compressed contents of the given file.
///
/// # Arguments
///
/// * `path` - A reference to the path of the file to compress and measure.
///
/// # Errors
///
/// Returns an `io::Error` if the file cannot be opened or read, or if compression fails.
fn zstd_compressed_size<P: AsRef<Path>>(path: P) -> io::Result<u64> {
    let mut input = File::open(path)?;
    let mut counter = CountingWriter::new();
    let mut encoder = zstd::stream::Encoder::new(&mut counter, 3)?;
    io::copy(&mut input, &mut encoder)?;
    encoder.finish()?;
    Ok(counter.bytes_written())
}

fn print_reports(reports: &[Report]) {
    // Determine the width of the first column based on the longest file name or header.
    let name_width = max(
        "File Name".len(),
        reports.iter().map(|r| r.file_name.len()).max().unwrap_or(0),
    );

    // Header row
    println!(
        "{:<name_width$}  {:<12} {:<12} {:<12} {:<12} {:<12} {:<15} {:<15}",
        "Dataset",      // first column
        "Throughput",   // 12 chars
        "Input (MB)",   // 12 chars
        "Output (MB)",  // 12 chars
        "Comp Ratio",   // 12 chars
        "Zstd (MB)",    // 12 chars
        "FB Dict (MB)", // 15 chars
        "Dict (MB)",    // 15 chars
        name_width = name_width
    );

    // Data rows
    for r in reports {
        let file_name = r.file_name.clone();
        let throughput = r.throughput;
        let input_mb = r.input_size_mb();
        let output_mb = r.output_size_mb();
        let ratio = r.compression_ratio();
        let zstd_size = r.zstd_compressed_size_mb();
        let catch_size = r.catch_all_dict_size_mb();
        let dict_size = r.dictionary_size_mb();
        println!(
            "{file_name:<name_width$}  {throughput:<12.2} {input_mb:<12.2} {output_mb:<12.2} {ratio:<12.0} {zstd_size:<12.2} {catch_size:<15.2} {dict_size:<15.2}"
        );
    }
}

/// Return the sum of `len()` for every regular file **directly inside** `dir`.
///
/// * Sub-directories, symlinks, sockets, etc. are ignored.
pub fn folder_size<P: AsRef<Path>>(dir: P) -> io::Result<u64> {
    let mut total: u64 = 0;
    for entry in fs::read_dir(dir)? {
        let md = entry?.metadata()?;
        if md.is_file() {
            total += md.len();
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
        let output_folder = Path::new(output_folder);
        let dict_size = fs::metadata(output_folder.join(DICTIONARY_NAME))
            .map(|md| md.len())
            .unwrap_or(0);
        let catch_all_dictionary_size = fs::metadata(output_folder.join(CATCH_ALL_DICTIONARY_NAME))
            .map(|md| md.len())
            .unwrap_or(0);

        reports.push(Report {
            file_name: Path::new(ndjson_file)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            throughput: (file_size as f64 / 1024.0 / 1024.0) / start_time.elapsed().as_secs_f64(),
            input_size: file_size,
            zstd_compressed_size: zstd_compressed_size(ndjson_file)?,
            output_size: folder_size(output_folder)?,
            catch_all_dictionary_size,
            dictionary_size: dict_size,
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
    if args.get(1) == Some(&"search".to_string()) {
        let search_term = args.get(2).expect("Search term is required");
        let output_folder = args.get(3).expect("Output folder is required");
        let searcher = Index::new(output_folder)
            .expect("Failed to create searcher")
            .searcher();
        let res = searcher
            .search_and_retrieve(search_term)
            .expect("Failed to search");
        for doc in res {
            println!("{doc}");
        }
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
