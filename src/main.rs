mod wifi;

use std::error::Error;
use std::time::Duration;
use futures::StreamExt;
use libp2p::{
    gossipsub, mdns, noise, swarm::NetworkBehaviour, swarm::SwarmEvent, tcp, yamux,
};
use tokio::time;
use serde::{Deserialize, Serialize};
use chrono::Utc;

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

#[derive(Serialize, Deserialize, Debug)]
struct ProbeAlert {
    peer_id: String,
    signal: u32,
    timestamp: i64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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

    let mut poll_interval = time::interval(Duration::from_secs(15));
    let peer_id_str = swarm.local_peer_id().to_string();

    println!("Local Peer ID: {:?}", peer_id_str);

    loop {
        tokio::select! {
            _ = poll_interval.tick() => {
                println!("Polling local WiFi stats...");
                match wifi::get_interface_data() {
                    Ok(stats) => {
                        println!("WiFi Stats: Signal {}%, Rx {} Mbps, Tx {} Mbps", 
                                 stats.signal, stats.receive_rate, stats.transmit_rate);
                        
                        // Threshold logic (e.g., alert if Signal < 70%)
                        if stats.signal < 70 {
                            let alert = ProbeAlert {
                                peer_id: peer_id_str.clone(),
                                signal: stats.signal,
                                timestamp: Utc::now().timestamp(),
                            };
                            let alert_json = serde_json::to_string(&alert)?;
                            if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), alert_json.as_bytes()) {
                                println!("Gossipsub publish error: {:?}", e);
                            } else {
                                println!("!!! Published Alert: Signal drop detected !!!");
                            }
                        }
                    }
                    Err(e) => println!("Error accessing WLAN API: {:?}", e),
                }
            }
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, _multiaddr) in list {
                        println!("mDNS discovered a new peer: {peer_id}");
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                    }
                },
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                    for (peer_id, _multiaddr) in list {
                        println!("mDNS discovered peer has expired: {peer_id}");
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
                        println!("!!! ALERT FROM PEER {} !!!", alert.peer_id);
                        println!("--- Signal level: {}%", alert.signal);
                        println!("--- Timestamp: {}", alert.timestamp);
                    } else {
                        println!("Received unknown Gossipsub message from {peer_id}: {msg_content}");
                    }
                },
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Local node is listening on {address}");
                }
                SwarmEvent::ConnectionClosed { peer_id, .. } => {
                    println!("Connection closed with peer: {peer_id}");
                    swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                }
                _ => {}
            }
        }
    }
}
