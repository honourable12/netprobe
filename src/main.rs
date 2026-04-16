mod wifi;
mod config;
mod app;
mod network;

use std::error::Error;
use std::io::stdout;
use tokio::sync::mpsc;
use serde::{Deserialize, Serialize};
use crossterm::{
    event::{Event, KeyCode, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, Terminal};
use std::time::Duration;
use futures::StreamExt;
use crate::app::App;
use crate::network::NetworkEvent;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProbeAlert {
    pub peer_id: String,
    pub signal: u32,
    pub avg_signal: f32,
    pub bssid: String,
    pub channel: u32,
    pub timestamp: i64,
}

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
    
    let mut app = App::new("Initializing...".to_string(), config);

    // 3. Spawn Network Task
    tokio::spawn(async move {
        if let Err(e) = network::run_network(tx.clone()).await {
            let _ = tx.send(NetworkEvent::Log(format!("Network Error: {:?}", e)));
        }
    });

    // 4. Main UI Loop
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
