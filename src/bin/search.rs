fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <output_folder> <query>", args[0]);
        std::process::exit(1);
    }
    let output_folder = &args[1];
    let query = &args[2];

    let searcher = moshiki::search::Searcher::new(output_folder).unwrap();
    let term_ids = searcher.search(query);
    println!("Found term IDs: {:?}", term_ids);
}
