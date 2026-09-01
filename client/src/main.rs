//! Interactive CLI tool for ESP-XY Bluetooth electronic leadscrew controller.

use std::time::Duration;

use clap::{Parser, Subcommand};
use esp_xy_client::{BleClient, BleConnection, Direction, Mode, RespLink};

#[derive(Parser, Debug)]
#[command(
    name = "esp-xy-client",
    version,
    about = "BLE Host Client for ESP-XY RISC-V Controller"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Scan for nearby ESP-XY devices advertising NUS
    Scan {
        #[arg(short, long, default_value = "5")]
        timeout_secs: u64,
    },
    /// Connect to device and query status
    Status {
        #[arg(short, long)]
        device: Option<String>,
    },
    /// Set operating mode (FEED, THREAD, JOG)
    Mode {
        mode: Mode,
        #[arg(short, long)]
        device: Option<String>,
    },
    /// Set electronic pitch ratio in micrometers
    Ratio {
        ratio_um: i64,
        #[arg(short, long)]
        device: Option<String>,
    },
    /// Set carriage direction (FWD, REV)
    Dir {
        direction: Direction,
        #[arg(short, long)]
        device: Option<String>,
    },
    /// Enable stepper drive
    Enable {
        #[arg(short, long)]
        device: Option<String>,
    },
    /// Disable stepper drive
    Disable {
        #[arg(short, long)]
        device: Option<String>,
    },
    /// Clear fault trip
    Clear {
        #[arg(short, long)]
        device: Option<String>,
    },
    /// Interactive monitor and command session
    Monitor {
        #[arg(short, long)]
        device: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let client = BleClient::new().await?;

    match cli.command {
        Commands::Scan { timeout_secs } => {
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
        }
        Commands::Status { device } => {
            let conn = connect_target(&client, device.as_deref()).await?;
            let status = conn
                .get_status(Duration::from_secs(2))
                .await?;
            println!("Status: {status:?}");
        }
        Commands::Mode { mode, device } => {
            let conn = connect_target(&client, device.as_deref()).await?;
            conn.set_mode(mode, Duration::from_secs(2))
                .await?;
            println!("Mode set to {mode}");
        }
        Commands::Ratio { ratio_um, device } => {
            let conn = connect_target(&client, device.as_deref()).await?;
            conn.set_ratio(ratio_um, Duration::from_secs(2))
                .await?;
            println!("Ratio set to {ratio_um} um");
        }
        Commands::Dir { direction, device } => {
            let conn = connect_target(&client, device.as_deref()).await?;
            conn.set_direction(direction, Duration::from_secs(2))
                .await?;
            println!("Direction set to {direction}");
        }
        Commands::Enable { device } => {
            let conn = connect_target(&client, device.as_deref()).await?;
            conn.enable(Duration::from_secs(2))
                .await?;
            println!("Stepper drive ENABLED");
        }
        Commands::Disable { device } => {
            let conn = connect_target(&client, device.as_deref()).await?;
            conn.disable(Duration::from_secs(2))
                .await?;
            println!("Stepper drive DISABLED");
        }
        Commands::Clear { device } => {
            let conn = connect_target(&client, device.as_deref()).await?;
            conn.clear_fault(Duration::from_secs(2))
                .await?;
            println!("Fault state CLEARED");
        }
        Commands::Monitor { device } => {
            let conn = connect_target(&client, device.as_deref()).await?;
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

async fn connect_target(
    client: &BleClient,
    target: Option<&str>,
) -> Result<BleConnection, Box<dyn std::error::Error>> {
    println!("Scanning for ESP-XY peripheral...");
    let devices = client
        .scan(Duration::from_secs(3))
        .await?;
    let found = match target {
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
    Ok(conn)
}
