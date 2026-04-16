use libp2p::{
    gossipsub, mdns, noise, swarm::NetworkBehaviour, swarm::SwarmEvent, tcp, yamux,
};
use std::error::Error;
use std::time::Duration;
use tokio::sync::mpsc;
use crate::{config, wifi, ProbeAlert};
use chrono::Utc;
use futures::StreamExt;

#[derive(NetworkBehaviour)]
pub struct MyBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
}

pub enum NetworkEvent {
    StatsUpdate(wifi::WifiStats, f32),
    AlertReceived(ProbeAlert),
    PeerDiscovered(String),
    PeerExpired(String),
    Log(String),
}

pub struct SignalHistory {
    values: Vec<u32>,
    window_size: usize,
}

impl SignalHistory {
    pub fn new(window_size: usize) -> Self {
        Self {
            values: Vec::new(),
            window_size,
        }
    }

    pub fn add(&mut self, val: u32) -> f32 {
        self.values.push(val);
        if self.values.len() > self.window_size {
            self.values.remove(0);
        }
        let sum: u32 = self.values.iter().sum();
        sum as f32 / self.values.len() as f32
    }
}

pub async fn run_network(
    tx: mpsc::UnboundedSender<NetworkEvent>,
) -> Result<(), Box<dyn Error>> {
    let config = config::load_config();
    let _ = tx.send(NetworkEvent::Log("Configuration loaded".to_string()));

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

    let mut poll_interval = tokio::time::interval(Duration::from_secs(config.poll_interval_secs));
    let peer_id_str = swarm.local_peer_id().to_string();
    let mut history = SignalHistory::new(config.moving_average_window);

    let _ = tx.send(NetworkEvent::Log(format!("Local Peer ID: {}", peer_id_str)));

    loop {
        tokio::select! {
            _ = poll_interval.tick() => {
                match wifi::get_interface_data() {
                    Ok(stats) => {
                        let avg_signal = history.add(stats.signal);
                        let _ = tx.send(NetworkEvent::StatsUpdate(stats.clone(), avg_signal));
                        
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
                                let _ = tx.send(NetworkEvent::Log(format!("Gossipsub publish error: {:?}", e)));
                            } else {
                                let _ = tx.send(NetworkEvent::Log(format!("Published Alert: Signal drop detected (Avg {:.1}%)", avg_signal)));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(NetworkEvent::Log(format!("Error accessing WLAN API: {:?}", e)));
                    }
                }
            }
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, _multiaddr) in list {
                        let p_id = peer_id.to_string();
                        let _ = tx.send(NetworkEvent::Log(format!("mDNS discovered a new peer: {}", p_id)));
                        let _ = tx.send(NetworkEvent::PeerDiscovered(p_id));
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                    }
                },
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                    for (peer_id, _multiaddr) in list {
                        let p_id = peer_id.to_string();
                        let _ = tx.send(NetworkEvent::Log(format!("mDNS discovered peer has expired: {}", p_id)));
                        let _ = tx.send(NetworkEvent::PeerExpired(p_id));
                        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                    }
                },
                SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source: peer_id,
                    message,
                    ..
                })) => {
                    let msg_content = String::from_utf8_lossy(&message.data);
                    if let Ok(alert) = serde_json::from_str::<ProbeAlert>(&msg_content) {
                        let _ = tx.send(NetworkEvent::AlertReceived(alert));
                    } else {
                        let _ = tx.send(NetworkEvent::Log(format!("Received unknown Gossipsub message from {}", peer_id)));
                    }
                },
                SwarmEvent::NewListenAddr { address, .. } => {
                    let _ = tx.send(NetworkEvent::Log(format!("Local node is listening on {}", address)));
                }
                SwarmEvent::ConnectionClosed { peer_id, .. } => {
                    let p_id = peer_id.to_string();
                    let _ = tx.send(NetworkEvent::Log(format!("Connection closed with peer: {}", p_id)));
                    let _ = tx.send(NetworkEvent::PeerExpired(p_id));
                    swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                }
                _ => {}
            }
        }
    }
}
