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

pub struct AppState {
    pub ble_fail_count: u32,
    pub ble_read_period_ms: u64,
    pub ble_inactivity_timeout_ms: u64,
    pub ble_inactivity_delay_ms: u64,
    pub verbose: bool,
}

#[derive(Deserialize, Debug)]
pub struct Info {
    pub Region: String,
    pub Model: String,
    pub HardwareVer: String,
    pub SoftwareVer: String,
    pub BootloaderVer: String,
    pub FileVer: String,
    pub SPCPVer: String,
    pub SN: String,
    pub CurTIME: String,
    pub CurBAT: String,
    pub CurBatState: String,
    pub CurOxiThr: String,
    pub CurMotor: String,
    pub CurPedtar: String,
    pub CurState: String,
    pub BranchCode: String,
    pub FileList: String,
    //version: u8, // Rust will automatically convert the JSON number to u8
}

pub const OXY_CMD_READ_START: u8 = 0x03;
pub const OXY_CMD_READ_CONTENT: u8 = 0x04;
pub const OXY_CMD_READ_END: u8 = 0x05;
pub const OXY_CMD_INFO: u8 = 0x14;
pub const OXY_CMD_PING: u8 = 0x15;
pub const OXY_CMD_PARA_SYNC: u8 = 0x16;
pub const OXY_CMD_RT_PARAM: u8 = 0x17; // LIVE_DATA
pub const OXY_CMD_FACTORY_RESET: u8 = 0x18;
pub const OXY_CMD_BURN_LOCK_FLASH: u8 = 0x19;
pub const OXY_CMD_BURN_FACTORY_INFO: u8 = 0x1A;
pub const OXY_CMD_RT_WAVE: u8 = 0x1B;
pub const OXY_CMD_PPG_RT_DATA: u8 = 0x1C;
pub const OXY_CMD_BOX_INFO: u8 = 0x1D;
pub const OXY_CMD_BOX_RE_MEASUREMENT: u8 = 0x40;

/// CRC8 code table
const TABLE_CRC8: [u8; 256] = [
    0x00, 0x07, 0x0E, 0x09, 0x1C, 0x1B, 0x12, 0x15, 0x38, 0x3F, 0x36, 0x31, 0x24, 0x23, 0x2A, 0x2D,
    0x70, 0x77, 0x7E, 0x79, 0x6C, 0x6B, 0x62, 0x65, 0x48, 0x4F, 0x46, 0x41, 0x54, 0x53, 0x5A, 0x5D,
    0xE0, 0xE7, 0xEE, 0xE9, 0xFC, 0xFB, 0xF2, 0xF5, 0xD8, 0xDF, 0xD6, 0xD1, 0xC4, 0xC3, 0xCA, 0xCD,
    0x90, 0x97, 0x9E, 0x99, 0x8C, 0x8B, 0x82, 0x85, 0xA8, 0xAF, 0xA6, 0xA1, 0xB4, 0xB3, 0xBA, 0xBD,
    0xC7, 0xC0, 0xC9, 0xCE, 0xDB, 0xDC, 0xD5, 0xD2, 0xFF, 0xF8, 0xF1, 0xF6, 0xE3, 0xE4, 0xED, 0xEA,
    0xB7, 0xB0, 0xB9, 0xBE, 0xAB, 0xAC, 0xA5, 0xA2, 0x8F, 0x88, 0x81, 0x86, 0x93, 0x94, 0x9D, 0x9A,
    0x27, 0x20, 0x29, 0x2E, 0x3B, 0x3C, 0x35, 0x32, 0x1F, 0x18, 0x11, 0x16, 0x03, 0x04, 0x0D, 0x0A,
    0x57, 0x50, 0x59, 0x5E, 0x4B, 0x4C, 0x45, 0x42, 0x6F, 0x68, 0x61, 0x66, 0x73, 0x74, 0x7D, 0x7A,
    0x89, 0x8E, 0x87, 0x80, 0x95, 0x92, 0x9B, 0x9C, 0xB1, 0xB6, 0xBF, 0xB8, 0xAD, 0xAA, 0xA3, 0xA4,
    0xF9, 0xFE, 0xF7, 0xF0, 0xE5, 0xE2, 0xEB, 0xEC, 0xC1, 0xC6, 0xCF, 0xC8, 0xDD, 0xDA, 0xD3, 0xD4,
    0x69, 0x6E, 0x67, 0x60, 0x75, 0x72, 0x7B, 0x7C, 0x51, 0x56, 0x5F, 0x58, 0x4D, 0x4A, 0x43, 0x44,
    0x19, 0x1E, 0x17, 0x10, 0x05, 0x02, 0x0B, 0x0C, 0x21, 0x26, 0x2F, 0x28, 0x3D, 0x3A, 0x33, 0x34,
    0x4E, 0x49, 0x40, 0x47, 0x52, 0x55, 0x5C, 0x5B, 0x76, 0x71, 0x78, 0x7F, 0x6A, 0x6D, 0x64, 0x63,
    0x3E, 0x39, 0x30, 0x37, 0x22, 0x25, 0x2C, 0x2B, 0x06, 0x01, 0x08, 0x0F, 0x1A, 0x1D, 0x14, 0x13,
    0xAE, 0xA9, 0xA0, 0xA7, 0xB2, 0xB5, 0xBC, 0xBB, 0x96, 0x91, 0x98, 0x9F, 0x8A, 0x8D, 0x84, 0x83,
    0xDE, 0xD9, 0xD0, 0xD7, 0xC2, 0xC5, 0xCC, 0xCB, 0xE6, 0xE1, 0xE8, 0xEF, 0xFA, 0xFD, 0xF4, 0xF3,
];

/// Generate CRC8 code
pub fn cal_crc8(buf: &[u8]) -> u8 {
    if buf.is_empty() {
        return 0;
    }

    let mut crc: u8 = 0;

    // Intentionally skip the last byte of the buffer.
    for &byte in buf.iter().take(buf.len().saturating_sub(1)) {
        let index = (crc ^ byte) as usize;
        crc = TABLE_CRC8[index];
    }
    crc
}

pub async fn wait_for_notifications(notification_stream: &mut Pin<Box<dyn Stream<Item = ValueNotification> + Send>>, buf: &mut Vec<u8>, number: i32) -> Result<(), Box<dyn Error>> {
    let mut counter = 0;
    let mut len1: usize = 99999;
    let mut offset1: usize = 0;
    let mut err = 1;
    while let Ok(Some(data)) = tokio::time::timeout(Duration::from_millis(2000), notification_stream.next()).await {
        if counter == 0 {
            if data.value[0] == 85 &&
                data.value[1] == 0 &&
                    data.value[2] == 255 {
                let bytes = [data.value[5], data.value[6]];
                len1 = u16::from_le_bytes(bytes) as usize;
            } else {
                println!("ERROR: Header not OK: {:#?}", data);
                continue;
            }
        }
        buf.extend(data.value.clone());
        offset1 += data.value.len();
        if offset1 >= (len1 + 8) {
            err = 0;
            break;
        }

        counter += 1;
        if counter >= 26 {
            err = 0;
            break; // This is just an optimisation so that it exits quickly, instead of waiting for
                   // the 100ms timeout.
        }
    }
    if err == 1 {
            println!("ERROR: Timeout");
            return Err("Timeout".into());
    }

    Ok(())
}

pub async fn get_info(peripheral: &Peripheral, write_char: &Characteristic, notification_stream_ref: &mut Pin<Box<dyn Stream<Item = ValueNotification> + Send>>, bufref1: &mut Vec<u8>) -> Result<(), Box<dyn Error>> {
    // Clear the buffer (Drain for 100ms)
    while let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(1000), notification_stream_ref.next()).await {
        // We just discard these packets
    }
    // Send request
    let mut write_bytes_info: Vec<u8> = Vec::new();
    let len = 0;
    // Expand it to size 8 filled with zeros
    write_bytes_info.resize(8 + len, 0);
    write_bytes_info[0] = 0xAA;
    write_bytes_info[1] = OXY_CMD_INFO;
    write_bytes_info[2] = !OXY_CMD_INFO; // Invert the bits.
    write_bytes_info[3] = 0x0;
    write_bytes_info[4] = 0x0;
    write_bytes_info[5] = (len & 0xff) as u8;
    write_bytes_info[6] = (len >> 8) as u8;
    write_bytes_info[7] = cal_crc8(&write_bytes_info);
    bufref1.clear();
    peripheral.write(write_char, &write_bytes_info, WriteType::WithResponse).await?;

    let result1 = wait_for_notifications(notification_stream_ref, bufref1, 26).await;

    let crc1 = cal_crc8(bufref1);
    if crc1 != bufref1[bufref1.len() - 1] {
        println!("get_info: CRC failed");
        return Err("CRC failed".into());
    } else {
    }

    Ok(())
}


pub fn get_info_buf_to_json(bufref1: &mut Vec<u8>, json3: &mut String) -> Result<(), Box<dyn Error>> {
    let mut json1 = bufref1[7..(bufref1.len() - 1)].to_vec();
    if let Some(pos) = json1.iter().rposition(|&x| x != 0) {
        json1.truncate(pos + 1);
    } else {
        json1.clear(); // Everything was a null byte
    }
    let mut s: &str = "";
    let mut json2 = match String::from_utf8(json1.clone()) {
        Ok(s) => s,
        Err(e) => "Error".to_string(),
    };
    *json3 = json2.trim().to_string();
    Ok(())
}

pub fn get_info_json_to_files(json3: &mut String, files2: &mut Vec<String>) -> Result<(), Box<dyn Error>> {
    let info1: Info = serde_json::from_str(json3)?;
    let files1: Vec<String> = info1.FileList
        .trim_end_matches(',')    // Remove that trailing comma
        .split(',')               // Split into an iterator of &str
        .map(|s| s.to_string())   // Convert each &str to an owned String
        .collect();               // Gather into a Vec

    *files2 = files1.clone();

    Ok(())
}


pub async fn read_file_start(state: &mut AppState, peripheral: &Peripheral, write_char: &Characteristic, notification_stream_ref: &mut Pin<Box<dyn Stream<Item = ValueNotification> + Send>>,
    filename1: &String, file_length1: &mut usize) -> Result<(), Box<dyn Error>> {

    // Send request
    let mut buf1: Vec<u8> = Vec::new();
    let mut write_bytes_read_start: Vec<u8> = Vec::new();
    let len = filename1.len();
    // Expand it to size 8 filled with zeros
    write_bytes_read_start.resize(8 + len + 1, 0);
    write_bytes_read_start[0] = 0xAA;
    write_bytes_read_start[1] = OXY_CMD_READ_START;
    write_bytes_read_start[2] = !OXY_CMD_READ_START; // Invert the bits.
    write_bytes_read_start[3] = 0x0;
    write_bytes_read_start[4] = 0x0;
    write_bytes_read_start[5] = ((len + 1) & 0xff) as u8;
    write_bytes_read_start[6] = ((len + 1) >> 8) as u8;
    let bytes1 = filename1.as_bytes();
    for i in 0..(len) {
        write_bytes_read_start[i + 7] = bytes1[i];
    }
    write_bytes_read_start[7 + len] = 0;
    let crc_offset = 8 + len ;
    write_bytes_read_start[crc_offset] = cal_crc8(&write_bytes_read_start);
    
    let seqNo: u32 = 1;
    let mtu = 20;
    for chunk in write_bytes_read_start.chunks(mtu) {
        peripheral.write(write_char, &chunk, WriteType::WithResponse).await?;
    }
    let mut result1 = wait_for_notifications(notification_stream_ref, &mut buf1, 1).await;
    if buf1.len() == 0 {
        sleep(Duration::from_millis(state.ble_read_period_ms)).await;
        for chunk in write_bytes_read_start.chunks(mtu) {
        peripheral.write(write_char, &chunk, WriteType::WithResponse).await?;
        }
        result1 = wait_for_notifications(notification_stream_ref, &mut buf1, 1).await;
    }

    let crc1 = cal_crc8(&buf1);
    if crc1 != buf1[buf1.len() - 1] {
        println!("get_info: CRC failed");
        return Err("CRC failed".into());
    } else {
    }

    if buf1.len() == 0 {
        println!("Read file size failed");
        return Err("Read file size failed".into());
    }
    if buf1[1] == 1 {
        println!("Read file size failed");
        return Err("Read file size failed".into());
    }

    let lsb1: u32 = buf1[7] as u32;
    let hsb1: u32 = buf1[8] as u32;
    let filesize = lsb1 + (hsb1 << 8);
    *file_length1 = filesize as usize;

    Ok(())
}

pub async fn read_file_contents(state: &mut AppState, peripheral: &Peripheral, write_char: &Characteristic, notification_stream_ref: &mut Pin<Box<dyn Stream<Item = ValueNotification> + Send>>,
    seqNo: u32, contents: &mut Vec<u8>) -> Result<(), Box<dyn Error>> {
    // Clear the buffer (Drain for 100ms)
    while let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(100), notification_stream_ref.next()).await {
        // We just discard these packets
    }
    let len2: usize = 0;
    let mut write_bytes_read_content: Vec<u8> = Vec::new();
    write_bytes_read_content.resize(8 + len2, 0);
    write_bytes_read_content[0] = 0xAA;
    write_bytes_read_content[1] = OXY_CMD_READ_CONTENT;
    write_bytes_read_content[2] = !OXY_CMD_READ_CONTENT; // Invert the bits.
    write_bytes_read_content[3] = (seqNo & 0xff) as u8;
    write_bytes_read_content[4] = (seqNo >> 8) as u8;
    write_bytes_read_content[5] = ((len2) & 0xff) as u8;
    write_bytes_read_content[6] = ((len2) >> 8) as u8;
    let crc_offset = 7 + len2 ;
    write_bytes_read_content[crc_offset] = cal_crc8(&write_bytes_read_content);

    // Send request
    let mtu = 20;
    for chunk in write_bytes_read_content.chunks(mtu) {
        peripheral.write(write_char, &chunk, WriteType::WithResponse).await?;
    }
    contents.clear();
    let mut result1 = wait_for_notifications(notification_stream_ref, contents, 2).await;
    if contents.len() == 0 {
        contents.clear();
        sleep(Duration::from_millis(state.ble_read_period_ms)).await;
        for chunk in write_bytes_read_content.chunks(mtu) {
            peripheral.write(write_char, &chunk, WriteType::WithResponse).await?;
        }
        result1 = wait_for_notifications(notification_stream_ref, contents, 2).await;
    }

    let crc1 = cal_crc8(contents);
    let crc2 = contents[contents.len() - 1];
    if crc1 != crc2 {
        println!("get_info: CRC failed. {} != {}", crc1, crc2);
        return Err("CRC failed".into());
    } else {
    }

    Ok(())
}

pub async fn read_file_end(peripheral: &Peripheral, write_char: &Characteristic, notification_stream_ref: &mut Pin<Box<dyn Stream<Item = ValueNotification> + Send>>) -> Result<(), Box<dyn Error>> {

    // Clear the buffer (Drain for 100ms)
    //sleep(Duration::from_millis(state.ble_read_period_ms)).await;
    while let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(100), notification_stream_ref.next()).await {
        // We just discard these packets
    }
    let len2: usize = 0;
    let mut buf1: Vec<u8> = Vec::new();
    let mut write_bytes_read_end: Vec<u8> = Vec::new();
    write_bytes_read_end.resize(8 + len2, 0);
    write_bytes_read_end[0] = 0xAA;
    write_bytes_read_end[1] = OXY_CMD_READ_END;
    write_bytes_read_end[2] = !OXY_CMD_READ_END; // Invert the bits.
    write_bytes_read_end[3] = 0;
    write_bytes_read_end[4] = 0;
    write_bytes_read_end[5] = ((len2) & 0xff) as u8;
    write_bytes_read_end[6] = ((len2) >> 8) as u8;
    let crc_offset = 7 + len2 ;
    write_bytes_read_end[crc_offset] = cal_crc8(&write_bytes_read_end);

    // Send request
    let mtu = 20;
    for chunk in write_bytes_read_end.chunks(mtu) {
        peripheral.write(write_char, &chunk, WriteType::WithResponse).await?;
    }
    buf1.clear();
    let result1 = wait_for_notifications(notification_stream_ref, &mut buf1, 2).await;

    let crc1 = cal_crc8(&buf1);
    let crc2 = buf1[buf1.len() - 1];
    if crc1 != crc2 {
        println!("read_end: CRC failed. {} != {}", crc1, crc2);
        return Err("CRC failed".into());
    } else {
    }

    Ok(())
}

pub async fn get_file(state: &mut AppState, peripheral: &Peripheral, write_char: &Characteristic, notification_stream_ref: &mut Pin<Box<dyn Stream<Item = ValueNotification> + Send>>,
    filename1: &String, file_contents1: &mut Vec<u8>) -> Result<(), Box<dyn Error>> {
    // Get file length and select the file.
    //
    // Get first segment. Check CRC. Store that is a sector vector.
    // If not enough bytes, get next segment, until bytes match.
    //
    // Clear the buffer (Drain for 100ms)
    while let Ok(Some(_)) = tokio::time::timeout(Duration::from_millis(2000), notification_stream_ref.next()).await {
        // We just discard these packets
    }
    // Send request
    let mut file_length1: usize = 0;
    let mut seqNo: u32 = 0;
    let mut contents1: Vec<u8> = Vec::new();
    let mut fileslice: Vec<Vec<u8>> = Vec::new();
    file_contents1.clear();
    contents1.clear();
    let mut offset: usize = 0;
    let result1 = read_file_start(state, peripheral, write_char, notification_stream_ref, filename1, &mut file_length1).await;
    //Removed OK: sleep(Duration::from_millis(state.ble_read_period_ms)).await;
    loop {
        let result2 = read_file_contents(state, peripheral, write_char, notification_stream_ref, seqNo, &mut contents1).await;
        if result2.is_err() {
            println!("ERROR: read_file_contents failed: {:#?}", result2);
            return result2;
        }
        if contents1.len() > 8 {
            println!("seqNo: {} file_length: {} data_len: {}", seqNo, contents1.len(), contents1.len() - 8);
        } else {
            println!("seqNo: {} file_length: {}", seqNo, contents1.len());
            return Err("File length zero. failed".into());
        }
        let slice1 = contents1[7..contents1.len() - 1].to_vec();
        let len = contents1.len() - 8;
        if len <= 512 { // Redo the read if > 512
            fileslice.push(slice1);
            offset = offset + len;
            if offset >= file_length1 {
                break;
            }
            seqNo = seqNo + 1;
        } else {
            println!("ERROR: slice length > 512. Retrying.");
        }
    }

    for slice in fileslice {
        file_contents1.extend(slice.clone());
    }

    let result3 = read_file_end(peripheral, write_char, notification_stream_ref).await;
    if file_contents1.len() != file_length1 {
        println!("File length too long actual: {} vs expected: {}", file_contents1.len(), file_length1);
        return Err("File length too long.".into());
    } else {
    }
    Ok(())
}

pub async fn sync_time(state: &mut AppState, peripheral: &Peripheral, write_char: &Characteristic, notification_stream_ref: &mut Pin<Box<dyn Stream<Item = ValueNotification> + Send>>,
    time_string2: &String) -> Result<(), Box<dyn Error>> {

    // Send request
    let mut buf1: Vec<u8> = Vec::new();
    let mut write_bytes_sync_time: Vec<u8> = Vec::new();
    let pre_amble = "{\"SetTIME\":\"".to_string();
    let post_amble = "\"}}".to_string();
    let mut all_time = pre_amble.clone();
    all_time.push_str(&time_string2);
    all_time.push_str(&post_amble);
    let mut bytes1 = all_time.as_bytes();
    let len = bytes1.len();
    // Expand it to size 8 filled with zeros
    write_bytes_sync_time.resize(8 + len + 1, 0);
    write_bytes_sync_time[0] = 0xAA;
    write_bytes_sync_time[1] = OXY_CMD_PARA_SYNC;
    write_bytes_sync_time[2] = !OXY_CMD_PARA_SYNC; // Invert the bits.
    write_bytes_sync_time[3] = 0x0;
    write_bytes_sync_time[4] = 0x0;
    write_bytes_sync_time[5] = ((len + 1) & 0xff) as u8;
    write_bytes_sync_time[6] = ((len + 1) >> 8) as u8;
    for i in 0..(len) {
        write_bytes_sync_time[i + 7] = bytes1[i];
    }
    write_bytes_sync_time[7 + len] = 0;
    let crc_offset = 8 + len ;
    write_bytes_sync_time[crc_offset] = cal_crc8(&write_bytes_sync_time);
    
    let seqNo: u32 = 1;
    let mtu = 20;
    for chunk in write_bytes_sync_time.chunks(mtu) {
        peripheral.write(write_char, &chunk, WriteType::WithResponse).await?;
    }
    let mut result1 = wait_for_notifications(notification_stream_ref, &mut buf1, 1).await;
    if buf1.len() == 0 {
        sleep(Duration::from_millis(state.ble_read_period_ms)).await;
        for chunk in write_bytes_sync_time.chunks(mtu) {
        peripheral.write(write_char, &chunk, WriteType::WithResponse).await?;
        }
        result1 = wait_for_notifications(notification_stream_ref, &mut buf1, 1).await;
    }

    let crc1 = cal_crc8(&buf1);
    if crc1 != buf1[buf1.len() - 1] {
        println!("synctime: CRC failed");
        return Err("CRC failed".into());
    } else {
        println!("synctime: CRC Ok");
    }

    if buf1.len() == 0 {
        println!("synctime: Read file size failed");
        return Err("Read file size failed".into());
    }
    if buf1[1] == 1 {
        println!("synctime: Read file size failed");
        return Err("Read file size failed".into());
    }

    Ok(())
}


pub async fn get_ppg(state: &mut AppState, peripheral: &Peripheral, write_char: &Characteristic, notification_stream_ref: &mut Pin<Box<dyn Stream<Item = ValueNotification> + Send>>,
    ) -> Result<(), Box<dyn Error>> {

    // Send request
	let mut ir1: Vec<i32> = Vec::new();
	let mut red1: Vec<i32> = Vec::new();
	let mut motion1: Vec<i32> = Vec::new();
    let mut buf1: Vec<u8> = Vec::new();
    let mut write_bytes_get_ppg: Vec<u8> = Vec::new();
    let mut bytes1: Vec<u8> = vec![1];
    let len = 1;
    // Expand it to size 8 filled with zeros
    write_bytes_get_ppg.resize(7 + len + 1, 0);
    write_bytes_get_ppg[0] = 0xAA;
    write_bytes_get_ppg[1] = OXY_CMD_PPG_RT_DATA;
    write_bytes_get_ppg[2] = !OXY_CMD_PPG_RT_DATA; // Invert the bits.
    write_bytes_get_ppg[3] = 0x0;
    write_bytes_get_ppg[4] = 0x0;
    write_bytes_get_ppg[5] = ((len) & 0xff) as u8;
    write_bytes_get_ppg[6] = ((len) >> 8) as u8;
    write_bytes_get_ppg[7] = 1;
    let crc_offset = 7 + len ;
    write_bytes_get_ppg[crc_offset] = cal_crc8(&write_bytes_get_ppg);
    
    let seqNo: u32 = 1;
    let mtu = 20;
    for chunk in write_bytes_get_ppg.chunks(mtu) {
        peripheral.write(write_char, &chunk, WriteType::WithResponse).await?;
    }
    let mut result1 = wait_for_notifications(notification_stream_ref, &mut buf1, 1).await;
    let crc1 = cal_crc8(&buf1);
    if crc1 != buf1[buf1.len() - 1] {
        println!("get_ppg: CRC failed");
        return Err("CRC failed".into());
    } else {
        println!("get_ppg: file_start: CRC Ok");
    }

    if buf1.len() == 0 {
        println!("get_ppg: Read file size failed");
        return Err("Read file size failed".into());
    }
    if buf1[1] == 1 {
        println!("get_ppg: Read file size failed");
        return Err("Read file size failed".into());
    }
	let mut off1 = 7;
	let mut i1 = 0;
        let mut bytes_ir1: [u8; 4] = [0u8; 4];

	let mut bytes_red1: Vec<u8> = Vec::new();
	let mut bytes_motion1: Vec<u8> = Vec::new();

	loop {
		off1 = (i1 * 9) + 11;
		if off1 >= buf1.len() {
			break;
		}
		// 4 bytes
		bytes_ir1[0] = buf1[off1];
		bytes_ir1[1] = buf1[off1 + 1];
		bytes_ir1[2] = buf1[off1 + 2];
		bytes_ir1[3] = buf1[off1 + 3];
		// 4 bytes
		bytes_red1.clear();
		bytes_red1.push(buf1[off1 + 4]);
		bytes_red1.push(buf1[off1 + 5]);
		bytes_red1.push(buf1[off1 + 6]);
		bytes_red1.push(buf1[off1 + 7]);
		// 1 bytes
		bytes_motion1.clear();
		bytes_motion1.push(buf1[off1 + 8]);
		
		let mut ir2:i32 = u32::from_le_bytes(bytes_ir1) as i32;
		let mut red2:i32 = u32::from_le_bytes(bytes_ir1) as i32;
		let mut motion2:i32 = u32::from_le_bytes(bytes_ir1) as i32;
		ir1.push(ir2);
		red1.push(red2);
		motion1.push(motion2);
		i1 += 1;
	}

	Ok(())
}

pub async fn get_rt_param(state: &mut AppState, peripheral: &Peripheral, write_char: &Characteristic, notification_stream_ref: &mut Pin<Box<dyn Stream<Item = ValueNotification> + Send>>,
    ) -> Result<(), Box<dyn Error>> {

    // Send request
    let mut ir1: Vec<i32> = Vec::new();
    let mut red1: Vec<i32> = Vec::new();
    let mut motion1: Vec<i32> = Vec::new();
    let mut buf1: Vec<u8> = Vec::new();
    let mut write_bytes_get_ppg: Vec<u8> = Vec::new();
    let mut bytes1: Vec<u8> = vec![1];
    let len = 0;
    // Expand it to size 8 filled with zeros
    write_bytes_get_ppg.resize(7 + len + 1, 0);
    write_bytes_get_ppg[0] = 0xAA;
    write_bytes_get_ppg[1] = OXY_CMD_RT_PARAM;
    write_bytes_get_ppg[2] = !OXY_CMD_RT_PARAM; // Invert the bits.
    write_bytes_get_ppg[3] = 0x0;
    write_bytes_get_ppg[4] = 0x0;
    write_bytes_get_ppg[5] = ((len) & 0xff) as u8;
    write_bytes_get_ppg[6] = ((len) >> 8) as u8;
    write_bytes_get_ppg[7] = 1;
    let crc_offset = 7 + len ;
    write_bytes_get_ppg[crc_offset] = cal_crc8(&write_bytes_get_ppg);
    
    let seqNo: u32 = 1;
    let mtu = 20;
    for chunk in write_bytes_get_ppg.chunks(mtu) {
        peripheral.write(write_char, &chunk, WriteType::WithResponse).await?;
    }

    let mut result1 = wait_for_notifications(notification_stream_ref, &mut buf1, 1).await;
    let crc1 = cal_crc8(&buf1);
    if crc1 != buf1[buf1.len() - 1] {
        println!("get_rt_param: CRC failed");
        return Err("CRC failed".into());
    } else {
        println!("get_rt_param: file_start: CRC Ok");
    }

    if buf1.len() == 0 {
        println!("get_rt_param: Read file size failed");
        return Err("Read file size failed".into());
    }
    if buf1[1] == 1 {
        println!("get_rt_param: Read file size failed");
        return Err("Read file size failed".into());
    }
    let mut off1 = 7;
    let mut i1 = 0;
    let mut bytes_ir1: [u8; 4] = [0u8; 4];
    let mut bytes_red1: [u8; 4] = [0u8; 4];
    let mut bytes_motion1: [u8; 1] = [0u8; 1];

    loop {
	off1 = (i1 * 9) + 11;
	if off1 >= buf1.len() {
            break;
	}
	// 4 bytes
	bytes_ir1[0] = buf1[off1];
	bytes_ir1[1] = buf1[off1 + 1];
	bytes_ir1[2] = buf1[off1 + 2];
	bytes_ir1[3] = buf1[off1 + 3];
	// 4 bytes
	bytes_red1[0] = buf1[off1 + 4];
	bytes_red1[1] = buf1[off1 + 5];
	bytes_red1[2] = buf1[off1 + 6];
	bytes_red1[3] = buf1[off1 + 7];
	// 1 bytes
	bytes_motion1[0] = buf1[off1 + 8];
	
	let mut ir2:i32 = u32::from_le_bytes(bytes_ir1) as i32;
	let mut red2:i32 = u32::from_le_bytes(bytes_red1) as i32;
	let mut motion2:i32 = u8::from_le_bytes(bytes_motion1) as i32;
	ir1.push(ir2);
	red1.push(red2);
	motion1.push(motion2);
	i1 += 1;
    }

    Ok(())
}


pub async fn get_rt_wave(state: &mut AppState, peripheral: &Peripheral, write_char: &Characteristic, notification_stream_ref: &mut Pin<Box<dyn Stream<Item = ValueNotification> + Send>>,
     buf2: &mut Vec<u8>
    ) -> Result<(), Box<dyn Error>> {

    // Send request
	let mut ir1: Vec<i32> = Vec::new();
	let mut red1: Vec<i32> = Vec::new();
	let mut motion1: Vec<i32> = Vec::new();
    let mut buf1: Vec<u8> = Vec::new();
    let mut write_bytes_get_ppg: Vec<u8> = Vec::new();
    let mut bytes1: Vec<u8> = vec![1];
    let len = 1;
    // Expand it to size 8 filled with zeros
    write_bytes_get_ppg.resize(7 + len + 1, 0);
    write_bytes_get_ppg[0] = 0xAA;
    write_bytes_get_ppg[1] = OXY_CMD_RT_WAVE;
    write_bytes_get_ppg[2] = !OXY_CMD_RT_WAVE; // Invert the bits.
    write_bytes_get_ppg[3] = 0x0;
    write_bytes_get_ppg[4] = 0x0;
    write_bytes_get_ppg[5] = ((len) & 0xff) as u8;
    write_bytes_get_ppg[6] = ((len) >> 8) as u8;
    //buf[7] = (byte) 0;  // 0 -> 125hz;  1-> 62.5hz

    write_bytes_get_ppg[7] = 0;
    let crc_offset = 7 + len ;
    write_bytes_get_ppg[crc_offset] = cal_crc8(&write_bytes_get_ppg);
    
    let seqNo: u32 = 1;
    let mtu = 20;
    for chunk in write_bytes_get_ppg.chunks(mtu) {
        peripheral.write(write_char, &chunk, WriteType::WithResponse).await?;
    }
    let mut result1 = wait_for_notifications(notification_stream_ref, &mut buf1, 1).await;
    let crc1 = cal_crc8(&buf1);
    if crc1 != buf1[buf1.len() - 1] {
        println!("get_rt_wave: CRC failed");
        return Err("CRC failed".into());
    } else {
        println!("get_rt_wave: CRC Ok");
    }

    if buf1.len() == 0 {
        println!("get_rt_wave: Read file size failed");
        return Err("Read file size failed".into());
    }
    if buf1[1] == 1 {
        println!("get_rt_wave: Read file size failed");
        return Err("Read file size failed".into());
    }
    let mut bytes_len1: [u8; 2] = [0u8; 2];
    bytes_len1[0] = buf1[17];
    bytes_len1[1] = buf1[18];
    let mut len2:u32 = u16::from_le_bytes(bytes_len1) as u32;
    for i in 0..len2 {
        buf2.push(buf1[19 + i as usize]);
    }

    Ok(())
}
