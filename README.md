# NetProbe: Distributed P2P Disruptor Detector

NetProbe is a lightweight, decentralized monitoring tool built in Rust. It runs as a background service on Windows machines to detect RF interference and WiFi performance drops across a local network using a P2P mesh architecture.

## Features

- **P2P Mesh Network**: Built with `libp2p`, allowing nodes to communicate without a central server.
- **Automatic Discovery**: Uses **mDNS** to automatically find and connect to other NetProbe nodes on the same LAN.
- **WiFi Health Monitoring**: Periodically polls the Windows WLAN API (via `netsh`) to track Signal Strength and Link Rates.
- **Real-time Alerts**: Utilizes **Gossipsub** to propagate "Interference Detected" alerts across the mesh when a node's signal quality drops below a threshold.
- **Triangulation Ready**: By sharing local noise levels, multiple nodes can help identify the physical location of a disruptor.

## How it Works

1. **Local Probe**: Each node runs `netsh wlan show interfaces` every 15 seconds.
2. **Threshold Check**: If the Signal Quality drops below 70%, the node considers the environment "disrupted."
3. **Gossip**: The affected node signs and broadcasts a JSON alert to the `disruptor-alerts` topic.
4. **Peer Awareness**: All nodes in the mesh receive the alert and log the Peer ID and signal level of the affected machine.

## Prerequisites

- **OS**: Windows (requires `netsh` utility).
- **WiFi**: A WiFi interface must be active and connected.
- **Rust**: [Rustup](https://rustup.rs/) (stable toolchain).

## Getting Started

### 1. Clone and Build
```powershell
git clone <repo-url>
cd netprobe
cargo build --release
```

### 2. Run
```powershell
./target/release/netprobe.exe
```

### 3. Firewall Setup
Ensure that the Windows Defender Firewall allows the binary to communicate. By default, it will listen on a random TCP port for P2P traffic. In a production environment, you may want to bind to a specific port (e.g., `4001`) and open it.

## Architecture

- **Transport**: TCP + QUIC.
- **Security**: Noise protocol (ED25519 keys).
- **Multiplexing**: Yamux.
- **Messaging**: Gossipsub.
- **Discovery**: mDNS.

## Monitoring Thresholds
The default threshold for an alert is **70% signal quality**. This can be adjusted in `src/main.rs` within the `poll_interval` loop.

## License
MIT
