//! TCP transport -- the network-mode counterpart to BLE, for devices joined to WiFi instead of
//! paired over Bluetooth. Same shape as [`crate::ble`]: only the two irreducible I/O primitives
//! of [`RespLink`] live here, everything else is shared.

use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::ToSocketAddrs;
use tokio::sync::{Mutex, mpsc};

use resp::{RespDecoder, RespFrame};

use crate::error::LinkError;
use crate::transport::RespLink;

/// Default port for the firmware's RESP-over-x TCP listener (a nod to RESP's usual home).
pub const DEFAULT_PORT: u16 = 6379;

/// Active connected TCP session with the ESP-Keiretsu device.
#[derive(Clone)]
pub struct TcpConnection {
    write_half: Arc<Mutex<OwnedWriteHalf>>,
    frame_rx: Arc<Mutex<mpsc::Receiver<RespFrame>>>,
}

impl TcpConnection {
    /// Connects to a device's RESP-over-x TCP listener.
    pub async fn connect(addr: impl ToSocketAddrs) -> Result<Self, LinkError> {
        let stream = TcpStream::connect(addr).await?;
        let (mut read_half, write_half) = stream.into_split();

        let (frame_tx, frame_rx) = mpsc::channel(64);

        // Background task to pump bytes off the socket into RESP frames. The decode itself
        // (RespDecoder) is sans-io; this task is just the async read loop feeding it.
        tokio::spawn(async move {
            let mut decoder = RespDecoder::new();
            let mut buf = [0u8; 256];
            loop {
                match read_half
                    .read(&mut buf)
                    .await
                {
                    Ok(0) | Err(_) => return,
                    Ok(n) => {
                        decoder.feed(&buf[..n]);
                        while let Some(frame) = decoder.next_frame() {
                            if frame_tx
                                .send(frame)
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            write_half: Arc::new(Mutex::new(write_half)),
            frame_rx: Arc::new(Mutex::new(frame_rx)),
        })
    }
}

impl RespLink for TcpConnection {
    async fn send_raw(&self, data: &[u8]) -> Result<(), LinkError> {
        self.write_half
            .lock()
            .await
            .write_all(data)
            .await?;
        Ok(())
    }

    async fn next_frame(&self) -> Result<RespFrame, LinkError> {
        let mut rx = self
            .frame_rx
            .lock()
            .await;
        rx.recv()
            .await
            .ok_or(LinkError::ChannelClosed)
    }
}
