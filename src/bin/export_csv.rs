// SPDX-License-Identifier: MIT OR Apache-2.0

//! Export the canonical CSV from a session's Parquet batches for corinth-canal.
//!
//! Accepts either a whole session directory (every batch, in order, one header) or
//! a single batch file. The column contract lives in `gaming_telemetry::export`.

use anyhow::Result;
use gaming_telemetry::export::{
    CANONICAL_COLUMNS, canonical_frame, resolve_inputs, to_csv, write_csv_atomically,
};
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: export_csv <session_dir | parquet_file> [output.csv]");
        eprintln!();
        eprintln!("  A directory exports every batch in it, in batch order, under a");
        eprintln!("  single header. A file exports just that batch.");
        eprintln!("  Output defaults to stdout (\"-\").");
        eprintln!();
        eprintln!("  Columns: {}", CANONICAL_COLUMNS.join(","));
        std::process::exit(1);
    }

    let input = Path::new(&args[1]);
    let output_file = args.get(2).map(String::as_str).unwrap_or("-");

    let inputs = resolve_inputs(input)?;
    let mut df = canonical_frame(&inputs)?;
    let csv = to_csv(&mut df)?;

    if output_file == "-" {
        print!("{csv}");
    } else {
        write_csv_atomically(Path::new(output_file), &csv)?;
        println!(
            "Exported {} rows from {} batch(es) to {}",
            df.height(),
            inputs.len(),
            output_file
        );
    }

    Ok(())
}
