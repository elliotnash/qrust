use crsf::{CrsfPacketParser, Packet, RcChannelMap};
use esp_idf_hal::uart::{AsyncUartDriver, UartDriver};
use tokio::sync::{Mutex, RwLock};

#[derive(Debug)]
pub struct Channels {
    pub roll: f32,
    pub pitch: f32,
    pub throttle: f32,
    pub yaw: f32,
}

pub struct Controller {
    uart: AsyncUartDriver<'static, UartDriver<'static>>,
    buffer: Mutex<[u8; 512]>,
    parser: Mutex<CrsfPacketParser>,
    pub channels: RwLock<Channels>,
}

impl Controller {
    pub fn new(uart: AsyncUartDriver<'static, UartDriver<'static>>) -> Self {
        Self {
            uart,
            buffer: Mutex::new([0; 512]),
            parser: Mutex::new(CrsfPacketParser::default()),
            channels: RwLock::new(Channels{
                roll: 0_f32,
                pitch: 0_f32,
                throttle: 0_f32,
                yaw: 0_f32,
            }),
        }
    }

    pub async fn read_loop(&self) -> anyhow::Result<()> {
        let mut buffer = self.buffer.lock().await;
        loop {
            let read = self.uart.read(&mut *buffer).await?;
            if read > 0 {
                let mut parser = self.parser.lock().await;
                parser.push_bytes(&buffer[..read]);
                while let Some(packet) = parser.next_packet() {
                    match packet {
                        Packet::LinkStatistics(_) => {}
                        Packet::RcChannelsPacked(channels) => {
                            let [roll, pitch, throttle, yaw, ..] = channels.get(RcChannelMap::float);

                            let mut channels = self.channels.write().await;
                            channels.roll = roll;
                            channels.pitch = pitch;
                            channels.throttle = throttle / 2_f32 + 0.5_f32;
                            channels.yaw = yaw;
                        }
                    }
                }
            }
        }
    }
}
