# NetProbe: Distributed P2P Disruptor Detector (TUI)

NetProbe is a lightweight, decentralized monitoring tool built in Rust. It runs as a background service on Windows machines to detect RF interference and WiFi performance drops across a local network using a P2P mesh architecture, now with a rich Terminal User Interface (TUI).

## Features

- **P2P Mesh Network**: Built with `libp2p`, allowing nodes to communicate without a central server.
- **Automatic Discovery**: Uses **mDNS** to automatically find and connect to other NetProbe nodes on the same LAN.
- **WiFi Health Monitoring**: Periodically polls the Windows WLAN API (via `netsh`) to track Signal Strength and Link Rates.
- **Hardware Sentinel Support**: Integrates with external ESP32-based "Silicon Sentinel" nodes via a built-in UDP bridge.
- **Real-time Alerts**: Utilizes **Gossipsub** to propagate "Interference Detected" alerts across the mesh when either a software or hardware node detects disruption.
- **Rich TUI Dashboard**: A real-time terminal interface powered by `ratatui` and `crossterm`.
    - **Signal Gauge**: Visual representation of current signal strength and moving average.
    - **Interface Info**: Detailed BSSID, Channel, and RX/TX rates.
    - **Peer List**: Real-time view of discovered nodes in the mesh.
    - **Alert History**: Scrollable history of received interference alerts from peers.
    - **Live Logs**: Application status and network event logs.

## How it Works

1. **Local Probe**: Each node polls `netsh wlan show interfaces` based on the configured interval.
2. **Threshold Check**: If the Signal Quality drops below the configured threshold, the node considers the environment "disrupted."
3. **Gossip**: The affected node signs and broadcasts a JSON alert to the `disruptor-alerts` topic.
4. **Peer Awareness**: All nodes in the mesh receive the alert and display it in the TUI's alert history.

## Prerequisites

- **OS**: 
  - **Windows**: Requires `netsh` utility (built-in).
  - **Linux**: Requires `nmcli` (NetworkManager) for WiFi monitoring.
- **WiFi**: A WiFi interface must be active and connected.
- **Rust**: [Rustup](https://rustup.rs/) (stable toolchain).

## Getting Started

### 1. Clone and Build
```bash
# Clone the repository
git clone <repo-url>
cd netprobe

# Build for your platform
cargo build --release
```

### 2. Configuration
On the first run, a `config.json` file will be created in the application directory. You can customize the following settings:
- `poll_interval_secs`: How often to check the WiFi status (default: 15).
- `signal_threshold`: The signal percentage below which an alert is triggered (default: 70).
- `moving_average_window`: Number of samples to use for the signal average (default: 3).

### 3. Run
```bash
# Windows
./target/release/netprobe.exe

# Linux
./target/release/netprobe
```
*Press **'q'** at any time to quit the TUI.*

### 4. Firewall Setup
Ensure that the Windows Defender Firewall allows the binary to communicate. It listens on a random TCP port for P2P traffic.

## Hardware Integration (Silicon Sentinel)

NetProbe supports dedicated hardware sensors built using the **ESP32** and **nRF24L01+**. These nodes perform hardware-level RF scanning and report interference directly to any NetProbe node via UDP.

- **UDP Bridge**: NetProbe listens on **UDP Port 4001** for incoming hardware sensor data.
- **Hardware Payload**: ESP32 Sentinels broadcast JSON payloads containing device ID, status, and detected RF anomalies.
- **Wiring & Design**: See [hardware_design.md](./hardware_design.md) for the complete schematic and component list.

## Architecture

- **UI Framework**: Ratatui + Crossterm.
- **Networking**: libp2p (TCP, QUIC, Noise, Yamux).
- **Messaging**: Gossipsub.
- **Discovery**: mDNS.
- **Runtime**: Tokio (Asynchronous).

## License
MIT
