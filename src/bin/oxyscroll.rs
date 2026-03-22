use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter, Characteristic};
use btleplug::platform::{Manager, Peripheral};
use chrono::Local;
use clap::Parser;
use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use futures_util::StreamExt;
use std::collections::VecDeque;
use std::error::Error;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use oxylib::*; // Assuming this provides AppState, get_rt_wave, etc.

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "OXY BLE Reader in Rust")]
struct Args {
    #[arg(short, long, default_value = "E6:8E:31:3E:50:10")]
    address: String,

    #[arg(short, long)]
    verbose: bool,

    #[arg(short, long)]
    scan: bool,
}

#[derive(Clone, Debug)]
struct BleDataPacket {
    waveform: Vec<u8>,
    spo2: u8,
    heart_rate: u8, // Using heart_rate for clarity, though you called it rt
}

// ----------------------------------------------------------------------------
// GUI Application State
// ----------------------------------------------------------------------------
struct BlePlotApp {
    rx: Receiver<BleDataPacket>, // Updated type
    data: VecDeque<[f64; 2]>,
    current_x: f64,
    window_size: usize,
    latest_spo2: u8,
    latest_hr: u8,
}

impl BlePlotApp {
    fn new(rx: Receiver<BleDataPacket>) -> Self {
        Self {
            rx,
            data: VecDeque::with_capacity(1000),
            current_x: 0.0,
            window_size: 1000,
            latest_spo2: 0,
            latest_hr: 0,
        }
    }
}

impl eframe::App for BlePlotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(packet) = self.rx.try_recv() {
            // Update numeric values
            self.latest_spo2 = packet.spo2;
            self.latest_hr = packet.heart_rate;

            // Update waveform
            for val in packet.waveform {
                self.data.push_back([self.current_x, val as f64]);
                self.current_x += 1.0;
                if self.data.len() > self.window_size {
                    self.data.pop_front();
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("OXY Live Monitor");

            // Graph Section
            let line = Line::new(PlotPoints::new(self.data.clone().into()));
            Plot::new("rt_wave_plot")
                .view_aspect(10.0)
                .show(ui, |plot_ui| plot_ui.line(line));

            ui.add_space(10.0);

            // Metrics Section (SpO2 and Heart Rate)
            ui.horizontal(|ui| {
                ui.group(|ui| {
                    ui.label(egui::RichText::new("SpO₂:").strong().size(24.0));
                    ui.label(egui::RichText::new(format!("{}%", self.latest_spo2)).color(egui::Color32::LIGHT_BLUE).size(24.0));
                });
                ui.add_space(20.0);
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Heart Rate:").strong().size(24.0));
                    ui.label(egui::RichText::new(format!("{} BPM", self.latest_hr)).color(egui::Color32::LIGHT_RED).size(24.0));
                });
            });

            ui.separator();

            // Time Section
            let time_str = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            ui.label(format!("System Time: {}", time_str));
        });

        ctx.request_repaint();
    }
}

// ----------------------------------------------------------------------------
// Main function (Setup GUI & spawn BLE thread)
// ----------------------------------------------------------------------------
fn main() -> Result<(), eframe::Error> {
    let args = Args::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Create a channel to send data from the BLE thread to the GUI thread
    let (tx, rx) = channel::<BleDataPacket>();

    // Spawn the Tokio runtime in a background thread
    let args_clone = args.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
        rt.block_on(async {
            if let Err(e) = run_ble_task(args_clone, tx).await {
                log::error!("BLE Task Error: {}", e);
            }
        });
    });

    // Start the eframe GUI on the main thread
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "OXY Real-time Graph",
        options,
        Box::new(|_cc| Box::new(BlePlotApp::new(rx))), // Remove Result wrapping for eframe 0.27
    )
}

// ----------------------------------------------------------------------------
// Async BLE Logic
// ----------------------------------------------------------------------------
async fn run_ble_task(args: Args, tx: Sender<BleDataPacket>) -> Result<(), Box<dyn Error>> {
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
        let peripherals = central.peripherals().await?;
        let peripheral = peripherals.into_iter()
            .find(|p| p.address().to_string().to_uppercase() == args.address.to_uppercase());

        if let Some(p) = peripheral {
            println!("Using Peripheral: {:#?}", p);
            if let Err(e) = run_device_loop(&p, &mut state, &tx).await {
                log::error!("Device loop error: {}", e);
            }
        } else {
            log::warn!("Device not found. Retrying...");
        }
        sleep(Duration::from_millis(state.ble_inactivity_delay_ms)).await;
    }
}

async fn run_device_loop(
    peripheral: &Peripheral,
    state: &mut AppState,
    tx: &Sender<BleDataPacket>
) -> Result<(), Box<dyn Error>> {

    peripheral.connect().await?;
    peripheral.discover_services().await?;
    log::info!("Connected to device!");

    let chars = peripheral.characteristics();
    let write_uuid = Uuid::parse_str("8b00ace7-eb0b-49b0-bbe9-9aee0a26e1a3")?;
    let notify_uuid = Uuid::parse_str("0734594a-a8e7-4b1a-a6b1-cd5243059a57")?; 

    let notify_char = chars.iter().find(|c| c.uuid == notify_uuid).ok_or("Notify char not found")?;
    let write_char = chars.iter().find(|c| c.uuid == write_uuid).ok_or("Write char not found")?;
    
    peripheral.subscribe(notify_char).await?;
    let mut notification_stream = peripheral.notifications().await?;

    // Clear the buffer
    while let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(100), notification_stream.next()).await {}

    let mut buf1: Vec<u8> = Vec::new();

    // Infinite loop pushing live data to the GUI
    loop {
        buf1.clear();
        let mut spo2 = 0;
        let mut hr = 0;
        
        // Fetch new data via oxylib
        get_rt_wave(state, peripheral, write_char, &mut notification_stream, &mut buf1, &mut spo2, &mut hr).await?;
        
        if !buf1.is_empty() {
            // Apply your sensor fixup logic locally to the incoming chunk
            apply_sensor_fixups(&mut buf1);

            let packet = BleDataPacket {
               waveform: buf1.clone(),
                spo2: spo2,
                heart_rate: hr,
            };

            // Send the fixed data to the GUI thread
            if tx.send(packet).is_err() {
                log::warn!("GUI channel closed. Shutting down BLE loop.");
                break;
            }
        }

        //sleep(Duration::from_millis(500)).await;
        sleep(Duration::from_millis(20)).await;
    }

    Ok(())
}

/// Extracts your sensor bug fixup logic so it can be applied to chunks on the fly
fn apply_sensor_fixups(data: &mut [u8]) {
    let len = data.len();
    if len < 2 { return; }

    for i in 0..len {
        if i == 0 {
            let after = data[i + 1];
            if data[i] > (after + 10) { data[i] = after; }
        } else if i < (len - 1) {
            let after = data[i + 1];
            if data[i] > (after + 10) {
                let before = data[i - 1];
                data[i] = ((before as u32 + after as u32) / 2) as u8;
            }
        } else if i == (len - 1) {
            let before = data[i - 1];
            if data[i] > (before + 10) { data[i] = before; }
        }
    }
    // Invert the data.
    for i in 0..len {
        data[i] = 255 - data[i];
    }
}

