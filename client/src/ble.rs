//! Bluetooth Low Energy (BLE) transport using Nordic UART Service (NUS).
//!
//! This module only implements the two irreducible I/O primitives of [`RespLink`] --
//! everything else (command semantics, reply parsing) lives in [`crate::protocol`] and
//! [`crate::transport`], shared with every other transport.

use std::sync::Arc;
use std::time::Duration;

use btleplug::api::{
    Central, Characteristic, Manager as _, Peripheral as _, ScanFilter, ValueNotification,
    WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use tokio::sync::{Mutex, mpsc};
use uuid::{Uuid, uuid};

use resp::{RespDecoder, RespFrame};

use crate::error::LinkError;
use crate::transport::RespLink;

/// Nordic UART Service (NUS) primary service UUID.
pub const NUS_SERVICE_UUID: Uuid = uuid!("6e400001-b5a3-f393-e0a9-e50e24dcca9e");
/// NUS RX Characteristic (Write / Write Without Response, Client -> ESP32-C6).
pub const NUS_RX_CHAR_UUID: Uuid = uuid!("6e400002-b5a3-f393-e0a9-e50e24dcca9e");
/// NUS TX Characteristic (Notify / Read, ESP32-C6 -> Client).
pub const NUS_TX_CHAR_UUID: Uuid = uuid!("6e400003-b5a3-f393-e0a9-e50e24dcca9e");

/// Discovered BLE peripheral candidate.
#[derive(Clone, Debug)]
pub struct DiscoveredDevice {
    pub peripheral: Peripheral,
    pub name: String,
    pub address: String,
}

/// High-level Bluetooth client manager.
pub struct BleClient {
    adapter: Adapter,
}

impl BleClient {
    /// Initializes BLE client with the first available Bluetooth adapter.
    pub async fn new() -> Result<Self, LinkError> {
        let manager = Manager::new().await?;
        let adapters = manager
            .adapters()
            .await?;
        let adapter = adapters
            .into_iter()
            .next()
            .ok_or(LinkError::NoAdapter)?;
        Ok(Self { adapter })
    }

    /// Scans for nearby BLE devices for the given duration.
    pub async fn scan(&self, timeout: Duration) -> Result<Vec<DiscoveredDevice>, LinkError> {
        self.adapter
            .start_scan(ScanFilter::default())
            .await?;
        tokio::time::sleep(timeout).await;
        self.adapter
            .stop_scan()
            .await?;

        let peripherals = self
            .adapter
            .peripherals()
            .await?;
        let mut discovered = Vec::new();

        for peripheral in peripherals {
            let properties = peripheral
                .properties()
                .await?
                .unwrap_or_default();
            let name = properties
                .local_name
                .unwrap_or_else(|| "Unknown".to_string());
            let address = peripheral
                .address()
                .to_string();

            // Check if device advertises NUS or matches typical name
            let has_nus = properties
                .services
                .contains(&NUS_SERVICE_UUID);
            if has_nus || name.contains("esp") || name.contains("leadscrew") || name != "Unknown" {
                discovered.push(DiscoveredDevice {
                    peripheral,
                    name,
                    address,
                });
            }
        }

        Ok(discovered)
    }

    /// Connects to a peripheral and negotiates NUS characteristics.
    pub async fn connect(&self, peripheral: &Peripheral) -> Result<BleConnection, LinkError> {
        if !peripheral
            .is_connected()
            .await?
        {
            peripheral
                .connect()
                .await?;
        }
        peripheral
            .discover_services()
            .await?;

        let chars = peripheral.characteristics();
        let rx_char = chars
            .iter()
            .find(|c| c.uuid == NUS_RX_CHAR_UUID)
            .cloned()
            .ok_or(LinkError::CharacteristicNotFound("NUS RX (Write)"))?;
        let tx_char = chars
            .iter()
            .find(|c| c.uuid == NUS_TX_CHAR_UUID)
            .cloned()
            .ok_or(LinkError::CharacteristicNotFound("NUS TX (Notify)"))?;

        peripheral
            .subscribe(&tx_char)
            .await?;
        let notifications = peripheral
            .notifications()
            .await?;

        let (frame_tx, frame_rx) = mpsc::channel(64);

        // Background task to process stream of incoming notifications into RESP frames. The
        // decode itself (RespDecoder) is sans-io; this task is just the async pump feeding it
        // bytes off the wire.
        tokio::spawn(async move {
            let mut decoder = RespDecoder::new();
            let mut stream = notifications;
            while let Some(ValueNotification { value, .. }) = stream
                .next()
                .await
            {
                decoder.feed(&value);
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
        });

        Ok(BleConnection {
            peripheral: peripheral.clone(),
            rx_char,
            frame_rx: Arc::new(Mutex::new(frame_rx)),
        })
    }
}

/// Active connected BLE session with the ESP-Keiretsu device.
#[derive(Clone)]
pub struct BleConnection {
    peripheral: Peripheral,
    rx_char: Characteristic,
    frame_rx: Arc<Mutex<mpsc::Receiver<RespFrame>>>,
}

impl BleConnection {
    /// Disconnects from the BLE peripheral.
    pub async fn disconnect(&self) -> Result<(), LinkError> {
        self.peripheral
            .disconnect()
            .await?;
        Ok(())
    }
}

impl RespLink for BleConnection {
    async fn send_raw(&self, data: &[u8]) -> Result<(), LinkError> {
        self.peripheral
            .write(&self.rx_char, data, WriteType::WithoutResponse)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nus_uuid_constants() {
        assert_eq!(
            NUS_SERVICE_UUID,
            Uuid::parse_str("6e400001-b5a3-f393-e0a9-e50e24dcca9e").unwrap()
        );
        assert_eq!(
            NUS_RX_CHAR_UUID,
            Uuid::parse_str("6e400002-b5a3-f393-e0a9-e50e24dcca9e").unwrap()
        );
        assert_eq!(
            NUS_TX_CHAR_UUID,
            Uuid::parse_str("6e400003-b5a3-f393-e0a9-e50e24dcca9e").unwrap()
        );
    }

    #[test]
    fn link_error_display() {
        assert_eq!(LinkError::NoAdapter.to_string(), "No Bluetooth adapter found");
        assert_eq!(
            LinkError::DeviceNotFound.to_string(),
            "Target device not found"
        );
        assert_eq!(
            LinkError::CharacteristicNotFound("NUS RX (Write)").to_string(),
            "NUS characteristic 'NUS RX (Write)' not found on device"
        );
        assert_eq!(LinkError::Timeout.to_string(), "Communication timed out");
        assert_eq!(LinkError::ChannelClosed.to_string(), "Channel closed");
        assert_eq!(
            LinkError::Protocol("corrupt frame".to_string()).to_string(),
            "Protocol error: corrupt frame"
        );
    }
}
