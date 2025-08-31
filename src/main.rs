mod config;
mod normalizer;
mod output;
mod parsers;

use anyhow::Result;
use clap::Parser;
use crossbeam::channel::unbounded;
use memchr::memchr_iter;
use memmap2::Mmap;
use rayon::prelude::*;
use std::fs::File;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    preset: String,

    #[arg(short, long, default_value = "stdout")]
    output: String,

    #[arg(value_name = "FILE")]
    file: String,

    #[arg(long, default_value = "1000000")]
    batch_size: usize,

    #[arg(long)]
    benchmark: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let start_time = Instant::now();
    let file_metadata = std::fs::metadata(&args.file)?;
    let file_size = file_metadata.len();

    // mmap the file
    let file = File::open(&args.file)?;
    let mmap = unsafe { Mmap::map(&file)? };

    // find newline offsets
    let line_positions: Vec<usize> = memchr_iter(b'\n', &mmap).collect();
    let total_lines = line_positions.len();

    // slice into batches
    let batches: Vec<&[u8]> = line_positions
        .chunks(args.batch_size)
        .map(|chunk| {
            let start = chunk.first().copied().unwrap_or(0);
            let end = chunk.last().copied().unwrap_or(mmap.len() - 1);
            &mmap[start..=end]
        })
        .collect();

    // channel for sending parsed batches to writer
    let (tx, rx) = crossbeam::channel::unbounded::<Vec<config::LogEntry>>();

    // spawn writer thread
    let output_arg = args.output.clone();
    let writer_handle = std::thread::spawn(move || {
        let mut writer = output::create_writer(&output_arg).unwrap();
        for batch in rx {
            writer.write_batch(&batch).unwrap();
        }
        writer.finish().unwrap();
    });

    // parallel parse batches
    let total_entries: usize = batches
        .par_iter()
        .map(|batch| {
            let s = unsafe { std::str::from_utf8_unchecked(batch) };
            let parsed = match parsers::parse(&args.preset, s) {
                Ok(parsed) => normalizer::normalize(parsed),
                Err(_) => Vec::new(),
            };
            let len = parsed.len();
            tx.send(parsed).unwrap();
            len
        })
        .sum();

    // close channel so writer thread can finish
    drop(tx);
    writer_handle.join().unwrap();

    if args.benchmark {
        print_benchmark_results(file_size, total_lines, total_entries, start_time.elapsed());
    }

    Ok(())
}

fn print_benchmark_results(
    file_size: u64,
    total_lines: usize,
    total_entries: usize,
    duration: std::time::Duration,
) {
    let duration_secs = duration.as_secs_f64();
    let file_size_mb = file_size as f64 / (1024.0 * 1024.0);
    let throughput_mbs = file_size_mb / duration_secs;
    let throughput_lines = total_lines as f64 / duration_secs;
    let throughput_entries = total_entries as f64 / duration_secs;

    eprintln!("\n=== BENCHMARK RESULTS ===");
    eprintln!("File size: {:.2} MB", file_size_mb);
    eprintln!("Total lines: {}", total_lines);
    eprintln!("Parsed entries: {}", total_entries);
    eprintln!("Processing time: {:.3}s", duration_secs);
    eprintln!("Throughput: {:.2} MB/s", throughput_mbs);
    eprintln!("Throughput: {:.0} lines/s", throughput_lines);
    eprintln!("Throughput: {:.0} entries/s", throughput_entries);
    eprintln!(
        "Parse success rate: {:.1}%",
        (total_entries as f64 / total_lines as f64) * 100.0
    );
}
