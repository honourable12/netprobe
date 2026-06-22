use std::process::Command;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiStats {
    pub signal: u32,
    pub receive_rate: f32,
    pub transmit_rate: f32,
    pub bssid: String,
    pub channel: u32,
}

#[cfg(target_os = "windows")]
pub fn get_interface_data() -> anyhow::Result<WifiStats> {
    let output = Command::new("netsh")
        .args(["wlan", "show", "interfaces"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    let signal_regex = Regex::new(r"Signal\s+:\s+(\d+)%")?;
    let rx_regex = Regex::new(r"Receive rate \(Mbps\)\s+:\s+([\d.]+)")?;
    let tx_regex = Regex::new(r"Transmit rate \(Mbps\)\s+:\s+([\d.]+)")?;
    let bssid_regex = Regex::new(r"BSSID\s+:\s+([0-9a-fA-F:]+)")?;
    let channel_regex = Regex::new(r"Channel\s+:\s+(\d+)")?;

    let signal = signal_regex.captures(&stdout)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().parse::<u32>().unwrap_or(0))
        .ok_or_else(|| anyhow::anyhow!("Could not find Signal strength"))?;

    let receive_rate = rx_regex.captures(&stdout)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().parse::<f32>().unwrap_or(0.0))
        .unwrap_or(0.0);

    let transmit_rate = tx_regex.captures(&stdout)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().parse::<f32>().unwrap_or(0.0))
        .unwrap_or(0.0);

    let bssid = bssid_regex.captures(&stdout)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let channel = channel_regex.captures(&stdout)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().parse::<u32>().unwrap_or(0))
        .unwrap_or(0);

    Ok(WifiStats {
        signal,
        receive_rate,
        transmit_rate,
        bssid,
        channel,
    })
}

#[cfg(target_os = "linux")]
pub fn get_interface_data() -> anyhow::Result<WifiStats> {
    let output = Command::new("nmcli")
        .args(["-t", "-f", "ACTIVE,SIGNAL,RATE,BSSID,CHAN", "dev", "wifi"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    let line_regex = Regex::new(r"^yes:(\d+):([^:]+):(.+):(\d+)$")?;
    
    for line in stdout.lines() {
        if let Some(caps) = line_regex.captures(line) {
            let signal = caps.get(1).map(|m| m.as_str().parse::<u32>().unwrap_or(0)).unwrap_or(0);
            let rate_str = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let bssid = caps.get(3).map(|m| m.as_str().replace("\\", "")).unwrap_or_else(|| "Unknown".to_string());
            let channel = caps.get(4).map(|m| m.as_str().parse::<u32>().unwrap_or(0)).unwrap_or(0);

            let rate_regex = Regex::new(r"([\d.]+)")?;
            let rate = rate_regex.captures(rate_str)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().parse::<f32>().unwrap_or(0.0))
                .unwrap_or(0.0);

            return Ok(WifiStats {
                signal,
                receive_rate: rate,
                transmit_rate: rate,
                bssid,
                channel,
            });
        }
    }

    Err(anyhow::anyhow!("No active WiFi connection found via nmcli"))
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn get_interface_data() -> anyhow::Result<WifiStats> {
    Err(anyhow::anyhow!("Platform not supported for WiFi monitoring"))
}


