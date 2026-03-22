use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter, WriteType, Characteristic};
use btleplug::platform::{Manager, Peripheral};
use btleplug::api::ValueNotification;
use chrono::Local;
use clap::Parser;
use std::error::Error;
use std::time::Duration;
use tokio::time::sleep;
use futures::Stream;
use futures_util::StreamExt;
use uuid::Uuid;
use std::pin::Pin;
use serde::Deserialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::collections::HashMap;
use oxylib::*;
use csv::Writer;
use plotters::prelude::*;


#[derive(Parser, Debug)]
#[command(author, version, about = "Viatom BLE Reader in Rust")]
struct Args {
    #[arg(short, long, default_value = "E6:8E:31:3E:50:10")]
    address: String,

    #[arg(short, long)]
    verbose: bool,

    #[arg(short, long)]
    scan: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;
    let central = adapters.into_iter().next().ok_or("No Bluetooth adapters found")?;
    let mut contents1: Vec<u8> = Vec::new();

    if args.scan {
        println!("Scanning for devices...");
        central.start_scan(ScanFilter::default()).await?;
        sleep(Duration::from_secs(10)).await;
        let devices = central.peripherals().await?;
        for p in devices {
            let properties = p.properties().await?.unwrap();
            println!("Device {:?} - Address: {}", properties.local_name, p.address());
        }
        return Ok(());
    }

    let mut state = AppState {
        ble_fail_count: 0,
        ble_read_period_ms: 1000,
        ble_inactivity_timeout_ms: 300000,
        ble_inactivity_delay_ms: 1000,
        verbose: args.verbose,
    };

    loop {
        log::info!("Connecting to device {}...", args.address);
        
        // Find peripheral by address
        let peripherals = central.peripherals().await?;
        let peripheral = peripherals.into_iter()
            .find(|p| p.address().to_string().to_uppercase() == args.address.to_uppercase());

        if let Some(p) = peripheral {
            println!("Using Peripheral: {:#?}",p);
            if let Err(e) = run_device_loop(&p, &mut state, &mut contents1).await {
                println!("Device error");
                log::error!("Device error: {}", e);
            }
        } else {
            log::warn!("Device not found. Retrying...");
        }
        break;
        log::info!("Waiting {}s to reconnect...", state.ble_inactivity_delay_ms);
        sleep(Duration::from_millis(state.ble_inactivity_delay_ms)).await;
    }

    for i in 0..contents1.len() {
        // Filter 156 values out due to sensor bugs.
        if i == 0 {
            let after = contents1[i + 1];
            if contents1[i] > (after + 10) {
                println!("Fixup1 contents1: {}: {}", i, contents1[i]);
                let middle = after;
                contents1[i] = middle;
            }
        } else if i < (contents1.len() - 1) {
            let after = contents1[i + 1];
            if contents1[i] > (after + 10) {
                let before = contents1[i - 1];
                let middle = ((before as u32 + after as u32) / 2 as u32) as u8;
                println!("Fixup2 contents1: {}: {}", i, contents1[i]);
                contents1[i] = middle;
                println!("Fixup2 contents1: {}: {}", i, contents1[i]);
            }
        } else if i == (contents1.len() - 1) {
            let before = contents1[i - 1];
            if contents1[i] > (before + 10) {
                println!("Fixup3 contents1: {}: {}", i, contents1[i]);
                let middle = before;
                contents1[i] = middle;
            }
        }
    }
    //println!("rt_wave2: {:#?}", contents1);
    let filename1 = "rtwave.csv";
    println!("filename: {}", filename1);
    
    let file_result = OpenOptions::new()
        .write(true)
        .create_new(true) 
        .open(&filename1);

    match file_result {
        Ok(mut file) => {
            let mut wtr = Writer::from_writer(file);
            println!("rt wave file: Len={}", contents1.len());
            for value in &contents1 {
                let value2 = *value as u32;
                wtr.serialize(value2)?;
            }       
            wtr.flush()?;
            println!("Successfully created and wrote to: {}", &filename1);
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
        println!("Skipping {}: File already exists.", &filename1);
        }
        Err(e) => return Err("Error creating file".into()), // Stop if there is a real error (like permission denied)
    }

    // Convert to (x, y) pairs: x = index, y = value as f32
    let points: Vec<(usize, f32)> = contents1.clone()
        .iter()
        .enumerate()
        .map(|(i, &v)| (i, v as f32))
        .collect();

    // Output file and size
    let out_file = "line_plot.png";
    let root = BitMapBackend::new(out_file, (1600, 960)).into_drawing_area();
    root.fill(&WHITE)?;

    // Determine x and y ranges
    let x_max = points.len().saturating_sub(1);
    let y_min = 0f32;
    let y_max = 255f32; // u8 max

    let mut chart = ChartBuilder::on(&root)
        .caption("Line plot from Vec<u8>", ("sans-serif", 30))
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(0usize..x_max, y_min..y_max)?;

    chart.configure_mesh().draw()?;

    chart.draw_series(LineSeries::new(
        points.into_iter().map(|(x, y)| (x, y)),
        &BLUE,
    ))?
    .label("value")
    .legend(|(x, y)| PathElement::new([(x, y), (x + 20, y)], &BLUE));

    chart.configure_series_labels().background_style(&WHITE.mix(0.8)).draw()?;

    root.present()?;
    println!("Saved {}", out_file);
    Ok(())
}

async fn run_device_loop(peripheral: &Peripheral, state: &mut AppState, contents1: &mut Vec<u8>) -> Result<(), Box<dyn Error>> {
    peripheral.connect().await?;
    peripheral.discover_services().await?;
    
    log::info!("Connected to device!");

    let chars = peripheral.characteristics();
    
    // magic stuff for the Viatom GATT service
    let write_uuid = Uuid::parse_str("8b00ace7-eb0b-49b0-bbe9-9aee0a26e1a3")?; // Example extension of your prefix
    let notify_uuid = Uuid::parse_str("0734594a-a8e7-4b1a-a6b1-cd5243059a57")?; 

    let notify_char = chars.iter().find(|c| c.uuid == notify_uuid).ok_or("Notify2 char not found")?;
    let write_char = chars.iter().find(|c| c.uuid == write_uuid).ok_or("Write2 char not found")?;
    
    // In Rust, we subscribe to notifications via a stream
    peripheral.subscribe(notify_char).await?;
    let mut notification_stream = peripheral.notifications().await?;

    // Clear the buffer (Drain for 100ms)
    while let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(100), notification_stream.next()).await {
        // We just discard these packets
    }   

    let mut buf1: Vec<u8> = Vec::new();
    let mut result1: Result<(), Box<dyn Error>>  = Ok(());
    let mut filenames1: Vec<String> = Vec::new();
    let mut file_contents1: Vec<u8> = Vec::new();
    
    buf1.clear();
    for i in 0..50 {
        let mut spo2: u8 = 0;
        let mut hr: u8 = 0;
        let result3 = get_rt_wave(state, peripheral, write_char, &mut notification_stream, &mut buf1, &mut spo2, &mut hr).await?;
        sleep(Duration::from_millis(500)).await;
    };
    contents1.clear();
    if contents1.capacity() < buf1.len() {
        contents1.reserve(buf1.len());
    }
    contents1.extend_from_slice(&buf1);

    Ok(())
}

