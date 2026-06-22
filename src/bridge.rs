use tokio::net::UdpSocket;
use std::io::Error;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SensorData {
    pub dev: String,
    pub status: String,
    pub alerts: Vec<RfAlert>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RfAlert {
    pub ch: u8,
    pub pwr: i16,
    pub r#type: String,
}

pub struct UdpBridge {
    socket: Arc<UdpSocket>,
}

impl UdpBridge {
    /// Creates a new bridge bound to a specific port (default 4001)
    pub async fn new(port: u16) -> Result<Self, Error> {
        let addr = format!("0.0.0.0:{}", port);
        let socket = UdpSocket::bind(&addr).await?;
        Ok(Self { socket: Arc::new(socket) })
    }

    pub fn local_addr(&self) -> Result<std::net::SocketAddr, Error> {
        self.socket.local_addr()
    }

    /// Listens for incoming UDP packets and returns parsed sensor data
    pub async fn listen_and_process(&self) -> Result<SensorData, Box<dyn std::error::Error>> {
        let mut buf = [0u8; 1024];
        
        loop {
            let (len, _addr) = self.socket.recv_from(&mut buf).await?;
            let raw_data = &buf[..len];

            // Attempt to parse the JSON payload from the ESP32
            match serde_json::from_slice::<SensorData>(raw_data) {
                Ok(data) => return Ok(data),
                Err(e) => {
                    // Log the error but keep the loop running
                    eprintln!("[Bridge] Received malformed packet: {:?}", e);
                }
            }
        }
    }
}
