//! Electronic leadscrew, BLE transport: drives a closed-loop stepper on the lathe carriage at
//! a fixed ratio of spindle rotation, read from a quadrature encoder via PCNT. Commandable at
//! runtime over BLE using a RESP-style protocol (see [`resp`]) carried over a Nordic UART
//! Service (NUS) GATT service -- any generic BLE terminal app (nRF Connect, "Serial Bluetooth
//! Terminal", ...) can drive it today; a proper app can be layered on later without changing
//! anything on this side, since it would speak the exact same protocol.
//!
//! The actual command engine and per-tick physics live in [`firmware::leadscrew`] and are
//! shared with the TCP transport binary (`leadscrew_tcp`) -- this file is just the BLE GATT
//! setup and the async I/O pump around that sans-io core.
//!
//! BLE integration note: `bleps`'s *sync* `AttributeServer` (`bleps::attribute_server`, not
//! `bleps::async_attribute_server`) is used deliberately -- it's driven by one
//! `do_work_with_notification()` call per control tick, which is non-blocking under the hood
//! (`esp_radio`'s `BleConnector::read()` drains whatever's immediately available and returns
//! rather than waiting), so BLE traffic can never stall the 1ms tick. No async executor is
//! needed or used here.
//!
//! On disconnect (`WorkResult::GotDisconnected`), advertising is restarted and the attribute
//! server is rebuilt so a new client can reconnect without a board reset. The control loop
//! itself does not depend on BLE at all -- if the link drops, the leadscrew simply keeps
//! executing its last commanded state until a client reconnects, which is the safe behavior.

#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use core::cell::RefCell;

use bleps::ad_structure::{
    AdStructure, BR_EDR_NOT_SUPPORTED, LE_GENERAL_DISCOVERABLE, create_advertising_data,
};
use bleps::attribute_server::{AttributeServer, NotificationData, WorkResult};
use bleps::no_rng::NoRng;
use bleps::{Ble, HciConnector, gatt};
use defmt::{error, info};
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::main;
use esp_hal::mcpwm::operator::PwmPinConfig;
use esp_hal::mcpwm::{McPwm, PeripheralClockConfig};
use esp_hal::pcnt::Pcnt;
use esp_hal::time::{Duration, Instant, Rate};
use esp_hal::timer::timg::TimerGroup;
use esp_println as _;
use esp_radio::ble::controller::BleConnector;
use firmware::leadscrew::{ControlLoop, Link, MAX_RPM_PITCH_UM, MICROSTEPS_PER_REV, SPINDLE_COUNTS_PER_REV, TICK_HZ};
use firmware::quadrature::QuadratureDecoder;
use firmware::stepper::StepGenerator;
use xiao_esp32c6_bsp::Board;

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    error!("{}", panic_info);
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

/// Control loop tick period, derived from [`TICK_HZ`].
const TICK: Duration = Duration::from_micros(1_000_000 / TICK_HZ as u64);

/// MCPWM timer period: with a 40MHz peripheral clock this gives a 200kHz ceiling and a
/// ~781Hz floor on the reconfigurable step rate, comfortably covering the demo's ~10kHz-ish
/// worst case with headroom to spare.
const MCPWM_PERIOD: u16 = 199;

// Nordic UART Service (NUS) UUIDs -- a de facto standard, recognized by many generic BLE
// terminal apps, which is the whole point: it lets a generic app be today's test client.
// Service: 6e400001-b5a3-f393-e0a9-e50e24dcca9e
// RX (write, client -> device): 6e400002-b5a3-f393-e0a9-e50e24dcca9e
// TX (notify, device -> client): 6e400003-b5a3-f393-e0a9-e50e24dcca9e
// (Used as string literals directly in the `gatt!` invocation below -- it needs literal
// tokens, not `const` references, since it expands at macro time.)

#[allow(
    clippy::large_stack_frames,
    reason = "BLE GATT setup allocates a handful of fixed buffers/closures inline; still well \
    within budget"
)]
#[main]
fn main() -> ! {
    let board = Board::new();

    let quad_pull = InputConfig::default().with_pull(Pull::Up);
    let spindle = QuadratureDecoder::new(
        Pcnt::new(
            board
                .remaining
                .pcnt,
        )
        .unit0,
        Input::new(board.d0, quad_pull),
        Input::new(board.d1, quad_pull),
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
    let mut control = ControlLoop::new(spindle, carriage);

    // BLE needs the scheduler running, same as the radio/Wi-Fi init pattern in main.rs.
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

    let connector = BleConnector::new(
        board
            .remaining
            .bt,
        Default::default(),
    )
    .expect("BLE radio init failed");
    let hci = HciConnector::new(connector, now_millis);
    let mut ble = Ble::new(&hci);
    ble.init()
        .expect("BLE HCI init failed");
    ble.cmd_set_le_advertising_parameters()
        .expect("failed to set advertising parameters");
    ble.cmd_set_le_advertising_data(
        create_advertising_data(&[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::CompleteLocalName("leadscrew"),
        ])
        .expect("advertising data too long"),
    )
    .expect("failed to set advertising data");
    ble.cmd_set_le_advertise_enable(true)
        .expect("failed to enable advertising");

    let link = RefCell::new(Link::new());

    let mut rx_write = |_offset: usize, data: &[u8]| {
        link.borrow_mut()
            .on_rx(data);
    };
    let mut tx_read = |offset: usize, data: &mut [u8]| {
        let link = link.borrow();
        let (buf, len) = &link.last_tx;
        let off = offset.min(*len);
        let n = data
            .len()
            .min(len - off);
        data[..n].copy_from_slice(&buf[off..off + n]);
        n
    };

    gatt!([service {
        uuid: "6e400001-b5a3-f393-e0a9-e50e24dcca9e",
        characteristics: [
            characteristic {
                name: "rx",
                uuid: "6e400002-b5a3-f393-e0a9-e50e24dcca9e",
                write: rx_write,
            },
            characteristic {
                name: "tx",
                uuid: "6e400003-b5a3-f393-e0a9-e50e24dcca9e",
                notify: true,
                read: tx_read,
            },
        ],
    },]);

    let mut rng = NoRng;

    info!(
        "leadscrew (BLE): {} counts/rev spindle, {} microsteps/rev, interlock {} RPM*um, \
        advertising as \"leadscrew\"",
        SPINDLE_COUNTS_PER_REV, MICROSTEPS_PER_REV, MAX_RPM_PITCH_UM
    );

    // Raw pointers so `AttributeServer` can be rebuilt from scratch each BLE session (see
    // below) without the borrow checker treating `ble`/`gatt_attributes`/`rng` as borrowed for
    // the rest of the function -- it can't reason about a loop-local borrow that's supposed to
    // end and restart every iteration, since `AttributeServer<'a, R>` ties all three inputs to
    // one invariant lifetime. Soundness is on us here: exactly one `AttributeServer` (and thus
    // at most one live `&mut` reborrow of each pointee) exists at a time, ended by an explicit
    // `drop(srv)` below before the pointers are ever dereferenced again.
    let ble_ptr: *mut Ble = &mut ble;
    let gatt_ptr: *mut _ = &mut gatt_attributes;
    let rng_ptr: *mut NoRng = &mut rng;

    // Outer loop: one iteration per BLE connection "session". `srv` is rebuilt fresh each time
    // around -- that's what lets `ble.cmd_set_le_advertise_enable` be called again once the
    // inner loop exits on disconnect, to make the device connectable again without a board
    // reset. All control-loop state above lives outside this loop, so a disconnect never
    // interrupts the leadscrew itself, only the BLE link.
    loop {
        let mut srv = AttributeServer::new(
            unsafe { &mut *ble_ptr },
            unsafe { &mut *gatt_ptr },
            unsafe { &mut *rng_ptr },
        );

        loop {
            let tick_start = Instant::now();

            // Drain one pending outbound frame (a command reply from last tick, or a push we
            // queued last tick) as this tick's notification, but only if the client has
            // actually subscribed -- and service any inbound command synchronously (may queue
            // a reply for *next* tick's send).
            let mut notification = None;
            let subscribed = {
                let mut cccd = [0u8; 1];
                matches!(
                    srv.get_characteristic_value(tx_notify_enable_handle, 0, &mut cccd),
                    Some(1)
                ) && cccd[0] == 1
            };
            if subscribed {
                let mut link_mut = link.borrow_mut();
                if let Some((buf, len)) = link_mut
                    .pending
                    .take()
                {
                    link_mut.last_tx = (buf, len);
                    notification = Some(NotificationData::new(
                        tx_handle,
                        &link_mut
                            .last_tx
                            .0[..len],
                    ));
                }
            }
            match srv.do_work_with_notification(notification) {
                Ok(WorkResult::GotDisconnected) => break,
                Ok(WorkResult::DidWork) => {}
                Err(_) => error!("BLE attribute server error"),
            }

            control.tick(&mut link.borrow_mut());

            while tick_start.elapsed() < TICK {}
        }

        // The inner loop only exits via disconnect (the `GotDisconnected` arm above). End
        // `srv`'s borrow of `ble`/`gatt_attributes` here (via a discarding move) rather than at
        // the (later) end of this block -- otherwise the borrow checker treats it as still
        // live through the `ble.cmd_set_le_advertise_enable` call below. `AttributeServer` has
        // no `Drop` impl, so `drop(srv)` would just trip clippy's `drop_non_drop` lint.
        let _ = srv;
        info!("BLE client disconnected, restarting advertising");
        if ble
            .cmd_set_le_advertise_enable(true)
            .is_err()
        {
            error!("failed to restart advertising after disconnect");
        }
    }
}

fn now_millis() -> u64 {
    Instant::now()
        .duration_since_epoch()
        .as_micros()
        / 1000
}
