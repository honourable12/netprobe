mod wifi;
mod config;
mod app;
mod network;
mod bridge;

use std::error::Error;
use std::io::stdout;
use tokio::sync::mpsc;
use crossterm::{
    event::{Event, KeyCode, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, Terminal};
use std::time::Duration;
use futures::StreamExt;
use crate::app::App;
use crate::network::{NetworkEvent, NetworkCommand};
use crate::bridge::UdpBridge;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 1. Setup Terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 2. Initialize App State
    let config = config::load_config();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<NetworkCommand>();
    
    let mut app = App::new("Initializing...".to_string(), config);

    // 3. Spawn Network Task
    let net_tx = tx.clone();
    tokio::spawn(async move {
        if let Err(e) = network::run_network(net_tx.clone(), cmd_rx).await {
            let _ = net_tx.send(NetworkEvent::Log(format!("Network Error: {:?}", e)));
        }
    });

    // 4. Spawn UDP Bridge Task (ESP32 Sentinel)
    let bridge_tx = tx.clone();
    let bridge_cmd_tx = cmd_tx.clone();
    tokio::spawn(async move {
        match UdpBridge::new(4001).await {
            Ok(bridge) => {
                let _ = bridge_tx.send(NetworkEvent::Log(format!("UDP Bridge listening on {}", bridge.local_addr().unwrap())));
                loop {
                    match bridge.listen_and_process().await {
                        Ok(sensor_data) => {
                            for alert in sensor_data.alerts {
                                let _ = bridge_tx.send(NetworkEvent::Log(format!("[Hardware] Alert from {}: {} on Ch {}", sensor_data.dev, alert.r#type, alert.ch)));
                                // Forward to network mesh
                                let _ = bridge_cmd_tx.send(NetworkCommand::PublishHardwareAlert {
                                    dev: sensor_data.dev.clone(),
                                    ch: alert.ch,
                                    pwr: alert.pwr,
                                    r#type: alert.r#type.clone(),
                                });
                            }
                        }
                        Err(e) => {
                            let _ = bridge_tx.send(NetworkEvent::Log(format!("Bridge Error: {:?}", e)));
                        }
                    }
                }
            }
            Err(e) => {
                let _ = bridge_tx.send(NetworkEvent::Log(format!("Failed to start UDP Bridge: {:?}", e)));
            }
        }
    });

    // 5. Main UI Loop
    let mut tick_interval = tokio::time::interval(Duration::from_millis(250));
    let mut events = EventStream::new();

    loop {
        terminal.draw(|f| app.ui(f))?;

        tokio::select! {
            _ = tick_interval.tick() => {
                // Background updates if any
            }
            event = rx.recv() => {
                if let Some(network_event) = event {
                    match network_event {
                        NetworkEvent::StatsUpdate(stats, avg) => {
                            app.wifi_stats = Some(stats);
                            app.avg_signal = avg;
                        }
                        NetworkEvent::AlertReceived(alert) => {
                            app.add_alert(alert);
                        }
                        NetworkEvent::PeerDiscovered(peer_id) => {
                            app.peers.insert(peer_id);
                        }
                        NetworkEvent::PeerExpired(peer_id) => {
                            app.peers.remove(&peer_id);
                        }
                        NetworkEvent::Log(msg) => {
                            if msg.contains("Local Peer ID:") {
                                app.local_peer_id = msg.replace("Local Peer ID: ", "");
                            }
                            app.add_log(msg);
                        }
                    }
                }
            }
            maybe_event = events.next() => {
                if let Some(Ok(Event::Key(key))) = maybe_event {
                    if let KeyCode::Char('q') = key.code {
                        app.should_quit = true;
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // 5. Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    
    Ok(())
}
