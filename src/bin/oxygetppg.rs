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
            if let Err(e) = run_device_loop(&p, &mut state).await {
                println!("Device error");
                log::error!("Device error: {}", e);
            }
        } else {
            log::warn!("Device not found. Retrying...");
        }
        break Ok(());
        log::info!("Waiting {}s to reconnect...", state.ble_inactivity_delay_ms);
        sleep(Duration::from_millis(state.ble_inactivity_delay_ms)).await;
    }
}

async fn run_device_loop(peripheral: &Peripheral, state: &mut AppState) -> Result<(), Box<dyn Error>> {
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
    let result3 = get_ppg(state, peripheral, write_char, &mut notification_stream).await?;

    Ok(())
}

