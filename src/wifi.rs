use std::process::Command;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiStats {
    pub signal: u32,
    pub receive_rate: f32,
    pub transmit_rate: f32,
}

pub fn get_interface_data() -> anyhow::Result<WifiStats> {
    let output = Command::new("netsh")
        .args(["wlan", "show", "interfaces"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    let signal_regex = Regex::new(r"Signal\s+:\s+(\d+)%")?;
    let rx_regex = Regex::new(r"Receive rate \(Mbps\)\s+:\s+([\d.]+)")?;
    let tx_regex = Regex::new(r"Transmit rate \(Mbps\)\s+:\s+([\d.]+)")?;

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

    Ok(WifiStats {
        signal,
        receive_rate,
        transmit_rate,
    })
}
