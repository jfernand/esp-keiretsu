//! Electronic leadscrew, TCP transport: same control loop as the BLE binary (`leadscrew`),
//! commandable instead over a plain TCP listener on the device's WiFi station address --
//! useful on a bench where the board is already on the shop WiFi and BLE proximity/pairing
//! would just be friction. Any TCP client speaking the resp-over-x wire format (the `client`
//! crate in this workspace, or a dumb `nc`/telnet session) works.
//!
//! The command engine and per-tick physics live in [`firmware::leadscrew`], shared verbatim
//! with the BLE binary -- this file is just WiFi/TCP setup and the async I/O pump around that
//! sans-io core, one accepted connection at a time (matching the BLE binary's one-session-at-a-
//! time model: a disconnect just means the leadscrew keeps running its last commanded state
//! until a client reconnects).
//!
//! WiFi credentials are compile-time env vars (`WIFI_SSID`/`WIFI_PASSWORD`) -- set them when
//! building, e.g. `WIFI_SSID=... WIFI_PASSWORD=... cargo build --bin leadscrew_tcp`.

#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use core::cell::RefCell;

use critical_section::Mutex;
use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_net::{Config, Runner, StackResources};
use embassy_time::{Duration, Instant, Timer};
use embedded_io_async::Write as _;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::mcpwm::operator::PwmPinConfig;
use esp_hal::mcpwm::{McPwm, PeripheralClockConfig};
use esp_hal::pcnt::Pcnt;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_println as _;
use esp_radio::wifi::sta::StationConfig;
use esp_radio::wifi::{self, Interface};
use firmware::leadscrew::{ControlLoop, Link, MAX_RPM_PITCH_UM, MICROSTEPS_PER_REV, SPINDLE_COUNTS_PER_REV, TICK_HZ};
use firmware::quadrature::QuadratureDecoder;
use firmware::stepper::StepGenerator;
use static_cell::StaticCell;
use xiao_esp32c6_bsp::Board;

/// Firmware's RESP-over-x TCP listener port (a nod to RESP's usual home).
const TCP_PORT: u16 = 6379;

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    error!("{}", panic_info);
    loop {}
}

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

/// MCPWM timer period: with a 40MHz peripheral clock this gives a 200kHz ceiling and a
/// ~781Hz floor on the reconfigurable step rate, comfortably covering the demo's ~10kHz-ish
/// worst case with headroom to spare.
const MCPWM_PERIOD: u16 = 199;

static LINK: Mutex<RefCell<Link>> = Mutex::new(RefCell::new(Link::new()));

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, Interface<'static>>) {
    runner
        .run()
        .await
}

#[embassy_executor::task]
async fn control_task(
    mut control: ControlLoop<'static, 0, esp_hal::peripherals::MCPWM0<'static>, 0, 0>,
) {
    let tick = Duration::from_hz(TICK_HZ as u64);
    loop {
        let tick_start = Instant::now();
        critical_section::with(|cs| {
            control.tick(&mut LINK.borrow_ref_mut(cs));
        });
        Timer::at(tick_start + tick).await;
    }
}

#[esp_hal::main]
fn main() -> ! {
    esp_alloc::heap_allocator!(size: 72 * 1024);

    let board = Board::new();

    let spindle = QuadratureDecoder::new(
        Pcnt::new(
            board
                .remaining
                .pcnt,
        )
        .unit0,
        Input::new(board.d0, InputConfig::default().with_pull(Pull::Up)),
        Input::new(board.d1, InputConfig::default().with_pull(Pull::Up)),
    );
    // No spindle index/Z pulse -- phase lock depends entirely on never losing a count, so
    // debounce contact/optical noise on the A/B lines.
    spindle
        .unit()
        .set_filter(Some(200))
        .expect("200 APB cycles is within the 1023 filter threshold limit");

    let mcpwm_clock = PeripheralClockConfig::with_frequency(Rate::from_mhz(40))
        .expect("40MHz is representable from the MCPWM source clock");
    let mut mcpwm = McPwm::new(
        board
            .remaining
            .mcpwm0,
        mcpwm_clock,
    );
    mcpwm
        .operator0
        .set_timer(&mcpwm.timer0);
    let step_pin = mcpwm
        .operator0
        .with_pin_a(board.d2, PwmPinConfig::UP_ACTIVE_HIGH);
    let dir_pin = Output::new(board.d3, Level::Low, OutputConfig::default());
    let carriage =
        StepGenerator::new(mcpwm_clock, mcpwm.timer0, step_pin, dir_pin, MCPWM_PERIOD);
    let control = ControlLoop::new(spindle, carriage);

    let timg0 = TimerGroup::new(
        board
            .remaining
            .timg0,
    );
    let sw_interrupt = SoftwareInterruptControl::new(
        board
            .remaining
            .sw_interrupt,
    );
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    let (controller, interfaces) = wifi::new(
        board
            .remaining
            .wifi,
        Default::default(),
    )
    .expect("WiFi radio init failed");

    static EXECUTOR: StaticCell<esp_rtos::embassy::Executor> = StaticCell::new();
    let executor = EXECUTOR.init(esp_rtos::embassy::Executor::new());
    executor.run(|spawner| {
        spawner.spawn(
            net_main(spawner, control, controller, interfaces.station)
                .expect("only one net_main is ever spawned"),
        );
    });
}

#[embassy_executor::task]
async fn net_main(
    spawner: Spawner,
    control: ControlLoop<'static, 0, esp_hal::peripherals::MCPWM0<'static>, 0, 0>,
    mut controller: wifi::WifiController<'static>,
    wifi_interface: Interface<'static>,
) {
    let station_config = wifi::Config::Station(
        StationConfig::default()
            .with_ssid(option_env!("WIFI_SSID").unwrap_or("change-me"))
            .with_password(
                option_env!("WIFI_PASSWORD")
                    .unwrap_or("change-me")
                    .into(),
            ),
    );
    controller
        .set_config(&station_config)
        .expect("failed to configure WiFi station");
    controller
        .connect_async()
        .await
        .expect("failed to connect to WiFi");
    info!("WiFi connected");

    static RESOURCES: StaticCell<StackResources<4>> = StaticCell::new();
    let resources = RESOURCES.init(StackResources::new());
    let (stack, runner) = embassy_net::new(
        wifi_interface,
        Config::dhcpv4(Default::default()),
        resources,
        // Fixed seed: fine for a bench tool on a trusted local network, not an
        // internet-facing service that needs unpredictable TCP initial sequence numbers.
        0x5eed_1234_5eed_1234,
    );
    spawner.spawn(net_task(runner).expect("only one net_task is ever spawned"));

    info!("waiting for DHCP lease...");
    stack
        .wait_config_up()
        .await;
    info!(
        "leadscrew (TCP): {} counts/rev spindle, {} microsteps/rev, interlock {} RPM*um, \
        listening on {}:{}",
        SPINDLE_COUNTS_PER_REV,
        MICROSTEPS_PER_REV,
        MAX_RPM_PITCH_UM,
        stack
            .config_v4()
            .map(|c| c.address.address()),
        TCP_PORT
    );

    spawner
        .spawn(control_task(control).expect("only one control_task is ever spawned"));

    let mut rx_buffer = [0u8; 256];
    let mut tx_buffer = [0u8; 256];
    loop {
        let mut socket = embassy_net::tcp::TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        if socket
            .accept(TCP_PORT)
            .await
            .is_err()
        {
            continue;
        }
        info!("TCP client connected");

        let mut read_buf = [0u8; 128];
        loop {
            // Drain one pending outbound frame (a command reply or a periodic status push)
            // every pass, then service one read -- mirrors the BLE binary's per-tick drain,
            // just paced by socket activity instead of a fixed spin-wait. The read is bounded
            // rather than a plain `.await` so a status push queued while the client is quiet
            // (e.g. just running `monitor`) still gets drained promptly instead of waiting on
            // the client to send something first.
            let pending = critical_section::with(|cs| {
                LINK.borrow_ref_mut(cs)
                    .pending
                    .take()
            });
            if let Some((buf, len)) = pending
                && socket
                    .write_all(&buf[..len])
                    .await
                    .is_err()
            {
                break;
            }

            match embassy_time::with_timeout(
                Duration::from_millis(50),
                socket.read(&mut read_buf),
            )
            .await
            {
                Err(_) => {} // timed out with nothing to read -- loop back to check `pending`
                Ok(Ok(0)) | Ok(Err(_)) => break,
                Ok(Ok(n)) => critical_section::with(|cs| {
                    LINK.borrow_ref_mut(cs)
                        .on_rx(&read_buf[..n]);
                }),
            }
        }

        info!("TCP client disconnected");
        socket.close();
    }
}
