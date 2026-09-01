//! The async edge around the sans-io protocol core in [`crate::protocol`]. Each backend (BLE,
//! TCP, ...) only implements two irreducible I/O primitives -- sending raw bytes and waiting
//! for the next decoded frame -- every device command below is the same orchestration
//! (encode via `protocol`, send, wait, parse via `protocol`) regardless of transport.

use std::time::Duration;

use resp::RespFrame;

use crate::error::LinkError;
use crate::protocol;
use crate::types::{Direction, Mode, Status};

/// A connected link to an ESP-XY device, carried over some byte transport.
#[allow(
    async_fn_in_trait,
    reason = "internal, non-dyn trait used only within this binary -- no need for an explicit \
    Send bound on the returned futures"
)]
pub trait RespLink {
    /// Sends raw, already-encoded bytes to the device.
    async fn send_raw(&self, data: &[u8]) -> Result<(), LinkError>;

    /// Receives the next incoming RESP frame (reply or push), already decoded.
    async fn next_frame(&self) -> Result<RespFrame, LinkError>;

    /// Sends a command and waits for the next reply frame within `timeout`.
    async fn send_command(
        &self,
        command: &str,
        timeout: Duration,
    ) -> Result<RespFrame, LinkError> {
        self.send_raw(&protocol::encode(command))
            .await?;
        tokio::time::timeout(timeout, self.next_frame())
            .await
            .map_err(|_| LinkError::Timeout)?
    }

    /// Queries device status via `STATUS` command.
    async fn get_status(&self, timeout: Duration) -> Result<Status, LinkError> {
        protocol::parse_status(
            self.send_command("STATUS", timeout)
                .await?,
        )
    }

    /// Sets operating mode (`MODE FEED`, `MODE THREAD`, or `MODE JOG`).
    async fn set_mode(&self, mode: Mode, timeout: Duration) -> Result<(), LinkError> {
        protocol::parse_ok(
            self.send_command(&protocol::mode_command(mode), timeout)
                .await?,
        )
    }

    /// Sets electronic pitch ratio in micrometers (`RATIO <um>`).
    async fn set_ratio(&self, ratio_um: i64, timeout: Duration) -> Result<(), LinkError> {
        protocol::parse_ok(
            self.send_command(&protocol::ratio_command(ratio_um), timeout)
                .await?,
        )
    }

    /// Sets stepper direction (`DIR FWD` or `DIR REV`).
    async fn set_direction(
        &self,
        direction: Direction,
        timeout: Duration,
    ) -> Result<(), LinkError> {
        protocol::parse_ok(
            self.send_command(&protocol::direction_command(direction), timeout)
                .await?,
        )
    }

    /// Enables the stepper drive (`ENABLE`).
    async fn enable(&self, timeout: Duration) -> Result<(), LinkError> {
        protocol::parse_ok(
            self.send_command("ENABLE", timeout)
                .await?,
        )
    }

    /// Disables the stepper drive (`DISABLE`).
    async fn disable(&self, timeout: Duration) -> Result<(), LinkError> {
        protocol::parse_ok(
            self.send_command("DISABLE", timeout)
                .await?,
        )
    }

    /// Clears any trip/fault state (`CLEAR`).
    async fn clear_fault(&self, timeout: Duration) -> Result<(), LinkError> {
        protocol::parse_ok(
            self.send_command("CLEAR", timeout)
                .await?,
        )
    }
}
