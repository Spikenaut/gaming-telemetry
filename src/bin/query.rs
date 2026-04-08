use anyhow::Result;
use duckdb::Connection;
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: cargo run --bin query <parquet_file>");
        return Ok(());
    }
    let parquet_file = &args[1];

    let conn = Connection::open_in_memory()?;

    println!("--- Analyzing {} with DuckDB ---", parquet_file);

    // Register Parquet extension if needed (usually included in modern duckdb crate)
    // conn.execute_batch("INSTALL parquet; LOAD parquet;")?;

    // Basic stats
    println!("\n[Summary Statistics]");
    let mut stmt = conn.prepare(&format!(
        "SELECT 
            avg(power_usage_mw) as avg_power, 
            max(power_usage_mw) as max_power,
            avg(temperature_c) as avg_temp,
            max(pcie_rx_kbps) as max_pcie_rx,
            max(pcie_tx_kbps) as max_pcie_tx,
            count(*) as sample_count
         FROM read_parquet('{}')", 
        parquet_file
    ))?;

    let mut rows = stmt.query([])?;
    if let Some(row) = rows.next()? {
        let avg_power: f64 = row.get(0)?;
        let max_power: u32 = row.get(1)?;
        let avg_temp: f64 = row.get(2)?;
        let max_rx: u32 = row.get(3)?;
        let max_tx: u32 = row.get(4)?;
        let count: i64 = row.get(5)?;

        println!("Samples: {}", count);
        println!("Avg Power: {:.2} W", avg_power / 1000.0);
        println!("Max Power: {:.2} W", max_power as f64 / 1000.0);
        println!("Avg Temp:  {:.1} C", avg_temp);
        println!("Max PCIe RX: {:.2} MB/s", max_rx as f64 / 1024.0);
        println!("Max PCIe TX: {:.2} MB/s", max_tx as f64 / 1024.0);
    }

    // Detecting "Inhibitory" Signals (Throttling)
    println!("\n[Throttling / Inhibitory Signals]");
    let mut stmt = conn.prepare(&format!(
        "SELECT timestamp_ms, throttle_reasons_bitmask 
         FROM read_parquet('{}') 
         WHERE throttle_reasons_bitmask != 0 
         LIMIT 5", 
        parquet_file
    ))?;

    let mut rows = stmt.query([])?;
    let mut found = false;
    while let Some(row) = rows.next()? {
        found = true;
        let ts: i64 = row.get(0)?;
        let mask: u64 = row.get(1)?;
        println!("TS: {} | Throttle Mask: {:016b}", ts, mask);
    }
    if !found {
        println!("No throttling events found in this batch.");
    }

    // Spikes (Excitatory)
    println!("\n[Potential PCIe Data Spikes]");
    let mut stmt = conn.prepare(&format!(
        "SELECT timestamp_ms, pcie_rx_kbps, power_usage_mw
         FROM read_parquet('{}') 
         ORDER BY pcie_rx_kbps DESC 
         LIMIT 5", 
        parquet_file
    ))?;

    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let ts: i64 = row.get(0)?;
        let rx: u32 = row.get(1)?;
        let pwr: u32 = row.get(2)?;
        println!("TS: {} | PCIe RX: {:6} KB/s | Power: {:5} mW", ts, rx, pwr);
    }

    Ok(())
}
