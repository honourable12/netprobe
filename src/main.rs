mod wifi;
mod config;

use std::error::Error;
use std::time::Duration;
use futures::StreamExt;
use libp2p::{
    gossipsub, mdns, noise, swarm::NetworkBehaviour, swarm::SwarmEvent, tcp, yamux,
};
use tokio::time;
use serde::{Deserialize, Serialize};
use chrono::Utc;
use tracing::{info, warn, error};
use tracing_subscriber;

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

#[derive(Serialize, Deserialize, Debug)]
struct ProbeAlert {
    peer_id: String,
    signal: u32,
    avg_signal: f32,
    bssid: String,
    channel: u32,
    timestamp: i64,
}

struct SignalHistory {
    values: Vec<u32>,
    window_size: usize,
}

impl SignalHistory {
    fn new(window_size: usize) -> Self {
        Self {
            values: Vec::new(),
            window_size,
        }
    }

    fn add(&mut self, val: u32) -> f32 {
        self.values.push(val);
        if self.values.len() > self.window_size {
            self.values.remove(0);
        }
        let sum: u32 = self.values.iter().sum();
        sum as f32 / self.values.len() as f32
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 5. Logging Framework
    tracing_subscriber::fmt::init();

    // 1. Configurable Thresholds
    let config = config::load_config();
    info!("Configuration loaded: {:?}", config);

    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        .with_behaviour(|key: &libp2p::identity::Keypair| {
            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(10))
                .validation_mode(gossipsub::ValidationMode::Strict)
                .build()
                .map_err(|msg| std::io::Error::new(std::io::ErrorKind::Other, msg))?;

            let gossipsub = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )?;

            let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())?;
            Ok(MyBehaviour { gossipsub, mdns })
        })?
        .with_swarm_config(|c: libp2p::swarm::Config| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    let topic = gossipsub::IdentTopic::new("disruptor-alerts");
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    let mut poll_interval = time::interval(Duration::from_secs(config.poll_interval_secs));
    let peer_id_str = swarm.local_peer_id().to_string();

    // 2. Moving Average
    let mut history = SignalHistory::new(config.moving_average_window);

    info!("Local Peer ID: {:?}", peer_id_str);

    loop {
        tokio::select! {
            // 3. Graceful Shutdown
            _ = tokio::signal::ctrl_c() => {
                info!("Shutdown signal received. Cleaning up...");
                break;
            }
            _ = poll_interval.tick() => {
                info!("Polling local WiFi stats...");
                match wifi::get_interface_data() {
                    Ok(stats) => {
                        let avg_signal = history.add(stats.signal);
                        info!("WiFi Stats: Signal {}% (Avg {:.1}%), BSSID {}, Channel {}", 
                                 stats.signal, avg_signal, stats.bssid, stats.channel);
                        
                        // Use Configurable Threshold
                        if avg_signal < config.signal_threshold as f32 {
                            let alert = ProbeAlert {
                                peer_id: peer_id_str.clone(),
                                signal: stats.signal,
                                avg_signal,
                                bssid: stats.bssid,
                                channel: stats.channel,
                                timestamp: Utc::now().timestamp(),
                            };
                            let alert_json = serde_json::to_string(&alert)?;
                            if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), alert_json.as_bytes()) {
                                error!("Gossipsub publish error: {:?}", e);
                            } else {
                                warn!("!!! Published Alert: Signal drop detected (Avg {:.1}%) !!!", avg_signal);
                            }
                        }
                    }
                    Err(e) => error!("Error accessing WLAN API: {:?}", e),
                }
            }
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, _multiaddr) in list {
                        info!("mDNS discovered a new peer: {peer_id}");
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                    }
                },
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                    for (peer_id, _multiaddr) in list {
                        info!("mDNS discovered peer has expired: {peer_id}");
                        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                    }
                },
                SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source: peer_id,
                    message_id: _id,
                    message,
                })) => {
                    let msg_content = String::from_utf8_lossy(&message.data);
                    if let Ok(alert) = serde_json::from_str::<ProbeAlert>(&msg_content) {
                        warn!("!!! ALERT FROM PEER {} !!!", alert.peer_id);
                        warn!("--- Signal level: {}% (Avg {:.1}%)", alert.signal, alert.avg_signal);
                        warn!("--- BSSID: {}, Channel: {}", alert.bssid, alert.channel);
                        warn!("--- Timestamp: {}", alert.timestamp);
                    } else {
                        info!("Received unknown Gossipsub message from {peer_id}: {msg_content}");
                    }
                },
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!("Local node is listening on {address}");
                }
                SwarmEvent::ConnectionClosed { peer_id, .. } => {
                    info!("Connection closed with peer: {peer_id}");
                    swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                }
                _ => {}
            }
        }
    }
    
    Ok(())
}
