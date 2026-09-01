//! Interactive CLI tool for the ESP-XY electronic leadscrew controller, over BLE or TCP.

use std::time::Duration;

use clap::{Parser, Subcommand};
use esp_xy_client::{AnyConnection, BleClient, Direction, Mode, RespLink, TcpConnection, tcp};

#[derive(Parser, Debug)]
#[command(
    name = "esp-xy-client",
    version,
    about = "Host Client for ESP-XY RISC-V Controller (BLE or TCP)"
)]
struct Cli {
    /// Connect over BLE to a device matching this name/address substring, instead of the
    /// first one found.
    #[arg(short, long, global = true)]
    device: Option<String>,
    /// Connect over TCP to this device instead of BLE (`host` or `host:port`, default port
    /// 6379).
    #[arg(long, global = true)]
    host: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Scan for nearby ESP-XY devices advertising NUS (BLE only)
    Scan {
        #[arg(short, long, default_value = "5")]
        timeout_secs: u64,
    },
    /// Connect to device and query status
    Status,
    /// Set operating mode (FEED, THREAD, JOG)
    Mode { mode: Mode },
    /// Set electronic pitch ratio in micrometers
    Ratio { ratio_um: i64 },
    /// Set carriage direction (FWD, REV)
    Dir { direction: Direction },
    /// Enable stepper drive
    Enable,
    /// Disable stepper drive
    Disable,
    /// Clear fault trip
    Clear,
    /// Interactive monitor and command session
    Monitor,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if let Commands::Scan { timeout_secs } = cli.command {
        let client = BleClient::new().await?;
        println!("Scanning for BLE devices ({timeout_secs}s)...");
        let devices = client
            .scan(Duration::from_secs(timeout_secs))
            .await?;
        if devices.is_empty() {
            println!("No devices found.");
        } else {
            println!("Found {} device(s):", devices.len());
            for (i, d) in devices
                .iter()
                .enumerate()
            {
                println!("  [{}] {} ({})", i + 1, d.name, d.address);
            }
        }
        return Ok(());
    }

    let conn = connect_target(cli.device.as_deref(), cli.host.as_deref()).await?;

    match cli.command {
        Commands::Scan { .. } => unreachable!("handled above"),
        Commands::Status => {
            let status = conn
                .get_status(Duration::from_secs(2))
                .await?;
            println!("Status: {status:?}");
        }
        Commands::Mode { mode } => {
            conn.set_mode(mode, Duration::from_secs(2))
                .await?;
            println!("Mode set to {mode}");
        }
        Commands::Ratio { ratio_um } => {
            conn.set_ratio(ratio_um, Duration::from_secs(2))
                .await?;
            println!("Ratio set to {ratio_um} um");
        }
        Commands::Dir { direction } => {
            conn.set_direction(direction, Duration::from_secs(2))
                .await?;
            println!("Direction set to {direction}");
        }
        Commands::Enable => {
            conn.enable(Duration::from_secs(2))
                .await?;
            println!("Stepper drive ENABLED");
        }
        Commands::Disable => {
            conn.disable(Duration::from_secs(2))
                .await?;
            println!("Stepper drive DISABLED");
        }
        Commands::Clear => {
            conn.clear_fault(Duration::from_secs(2))
                .await?;
            println!("Fault state CLEARED");
        }
        Commands::Monitor => {
            println!("Connected! Streaming notifications and status pushes (Ctrl+C to stop)...");
            loop {
                let frame = conn
                    .next_frame()
                    .await?;
                println!("< {frame:?}");
            }
        }
    }

    Ok(())
}

/// Connects to a device over TCP (if `--host` was given) or BLE (scan + connect, matching
/// `device` if given), returning a backend-agnostic [`AnyConnection`].
async fn connect_target(
    device: Option<&str>,
    host: Option<&str>,
) -> Result<AnyConnection, Box<dyn std::error::Error>> {
    if let Some(host) = host {
        let addr = if host.contains(':') {
            host.to_string()
        } else {
            format!("{host}:{}", tcp::DEFAULT_PORT)
        };
        println!("Connecting to {addr} over TCP...");
        let conn = TcpConnection::connect(addr).await?;
        println!("Connected.");
        return Ok(AnyConnection::Tcp(conn));
    }

    let client = BleClient::new().await?;
    println!("Scanning for ESP-XY peripheral...");
    let devices = client
        .scan(Duration::from_secs(3))
        .await?;
    let found = match device {
        Some(t) => devices
            .into_iter()
            .find(|d| {
                d.name
                    .contains(t)
                    || d.address
                        .contains(t)
            })
            .ok_or_else(|| format!("device matching '{t}' not found"))?,
        None => devices
            .into_iter()
            .next()
            .ok_or_else(|| "no BLE devices found".to_string())?,
    };

    println!("Connecting to {} ({})...", found.name, found.address);
    let conn = client
        .connect(&found.peripheral)
        .await?;
    println!("Connected and subscribed to NUS notifications.");
    Ok(AnyConnection::Ble(conn))
}
