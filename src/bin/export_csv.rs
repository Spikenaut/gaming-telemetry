//! Export canonical 5-column CSV from Parquet for corinth-canal ingestion.
//!
//! Canonical format: timestamp_ms,gpu_temp_c,gpu_power_w,cpu_tctl_c,cpu_package_power_w

use polars::prelude::*;
use std::env;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --bin export_csv <parquet_file> [output.csv]");
        eprintln!("  Exports canonical 5-column CSV for corinth-canal ingestion.");
        std::process::exit(1);
    }

    let parquet_file = &args[1];
    let output_file = args.get(2).map(|s| s.as_str()).unwrap_or("-");

    // Read Parquet
    let df = LazyFrame::scan_parquet(parquet_file, ScanArgsParquet::default())?
        .select(&[
            col("timestamp_ms"),
            col("temperature_c").alias("gpu_temp_c"),
            (col("power_usage_mw") / lit(1000.0)).alias("gpu_power_w"),
            col("cpu_tctl_c"),
            col("cpu_package_power_w"),
        ])
        .collect()?;

    // Write CSV
    let mut csv_buffer = Vec::new();
    CsvWriter::new(&mut csv_buffer)
        .include_header(true)
        .finish(&mut df.clone())?;

    let csv_string = String::from_utf8(csv_buffer)?;

    if output_file == "-" {
        println!("{}", csv_string);
    } else {
        std::fs::write(output_file, csv_string)?;
        println!("Exported {} rows to {}", df.height(), output_file);
    }

    Ok(())
}
