use anyhow::Result;
use chrono::{DateTime, Utc};
use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};
use nvml_wrapper::Nvml;
use polars::prelude::*;
use std::fs::File;
use std::sync::Arc;
use tokio::time::{interval, Duration};

#[derive(Debug, Clone)]
struct GpuSample {
    timestamp: DateTime<Utc>,
    power_usage_mw: u32,
    temperature_c: u32,
    graphics_clock_mhz: u32,
    memory_clock_mhz: u32,
    pcie_rx_throughput_kbps: u32,
    pcie_tx_throughput_kbps: u32,
    pstate: u32,
    throttle_reasons: u64,
    fan_speed_perc: u32,
    memory_used_mb: u64,
    memory_total_mb: u64,
}

const BUFFER_SIZE: usize = 2000; // ~10 seconds of data at 5ms intervals
const POLL_INTERVAL_MS: u64 = 5;

async fn write_to_parquet(samples: Vec<GpuSample>, batch_id: u32) -> Result<()> {
    let timestamps: Vec<i64> = samples.iter().map(|s| s.timestamp.timestamp_millis()).collect();
    let power: Vec<u32> = samples.iter().map(|s| s.power_usage_mw).collect();
    let temp: Vec<u32> = samples.iter().map(|s| s.temperature_c).collect();
    let graphics_clock: Vec<u32> = samples.iter().map(|s| s.graphics_clock_mhz).collect();
    let memory_clock: Vec<u32> = samples.iter().map(|s| s.memory_clock_mhz).collect();
    let pcie_rx: Vec<u32> = samples.iter().map(|s| s.pcie_rx_throughput_kbps).collect();
    let pcie_tx: Vec<u32> = samples.iter().map(|s| s.pcie_tx_throughput_kbps).collect();
    let pstate: Vec<u32> = samples.iter().map(|s| s.pstate).collect();
    let throttle: Vec<u64> = samples.iter().map(|s| s.throttle_reasons).collect();
    let fan: Vec<u32> = samples.iter().map(|s| s.fan_speed_perc).collect();
    let mem_used: Vec<u64> = samples.iter().map(|s| s.memory_used_mb).collect();
    let mem_total: Vec<u64> = samples.iter().map(|s| s.memory_total_mb).collect();

    let mut df = df!(
        "timestamp_ms" => timestamps,
        "power_usage_mw" => power,
        "temperature_c" => temp,
        "graphics_clock_mhz" => graphics_clock,
        "memory_clock_mhz" => memory_clock,
        "pcie_rx_kbps" => pcie_rx,
        "pcie_tx_kbps" => pcie_tx,
        "pstate" => pstate,
        "throttle_reasons_bitmask" => throttle,
        "fan_speed_perc" => fan,
        "memory_used_mb" => mem_used,
        "memory_total_mb" => mem_total,
    )?;

    let filename = format!("gpu_telemetry_batch_{}.parquet", batch_id);
    let file = File::create(&filename)?;
    ParquetWriter::new(file).finish(&mut df)?;
    
    println!("Wrote batch {} to {}", batch_id, filename);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let nvml = Arc::new(Nvml::init()?);
    let device = nvml.device_by_index(0)?; // Target RTX 5080 (index 0)
    
    let mut buffer = Vec::with_capacity(BUFFER_SIZE);
    let mut interval = interval(Duration::from_millis(POLL_INTERVAL_MS));
    let mut batch_counter = 0;

    println!("Starting expanded GPU telemetry polling every {}ms...", POLL_INTERVAL_MS);

    loop {
        interval.tick().await;

        let power_usage = device.power_usage().unwrap_or(0);
        let temperature = device.temperature(TemperatureSensor::Gpu).unwrap_or(0);
        let graphics_clock = device.clock_info(Clock::Graphics).unwrap_or(0);
        let memory_clock = device.clock_info(Clock::Memory).unwrap_or(0);
        
        let pcie_rx = device.pcie_throughput(nvml_wrapper::enum_wrappers::device::PcieUtilCounter::Receive).unwrap_or(0);
        let pcie_tx = device.pcie_throughput(nvml_wrapper::enum_wrappers::device::PcieUtilCounter::Send).unwrap_or(0);
        let pstate = device.performance_state().map(|p| p as u32).unwrap_or(0);
        let throttle = device.current_throttle_reasons().map(|t| t.bits()).unwrap_or(0);
        let fan = device.fan_speed(0).unwrap_or(0);
        let mem_info = device.memory_info();
        
        let sample = GpuSample {
            timestamp: Utc::now(),
            power_usage_mw: power_usage,
            temperature_c: temperature,
            graphics_clock_mhz: graphics_clock,
            memory_clock_mhz: memory_clock,
            pcie_rx_throughput_kbps: pcie_rx,
            pcie_tx_throughput_kbps: pcie_tx,
            pstate,
            throttle_reasons: throttle,
            fan_speed_perc: fan,
            memory_used_mb: mem_info.as_ref().map(|m| m.used / 1024 / 1024).unwrap_or(0),
            memory_total_mb: mem_info.as_ref().map(|m| m.total / 1024 / 1024).unwrap_or(0),
        };

        buffer.push(sample);

        if buffer.len() >= BUFFER_SIZE {
            let samples_to_write = std::mem::replace(&mut buffer, Vec::with_capacity(BUFFER_SIZE));
            batch_counter += 1;
            
            // Write asynchronously to avoid blocking the polling loop
            tokio::spawn(async move {
                if let Err(e) = write_to_parquet(samples_to_write, batch_counter).await {
                    eprintln!("Failed to write to Parquet: {:?}", e);
                }
            });
        }
    }
}
