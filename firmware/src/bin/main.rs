#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use defmt::{error, info};
use esp_hal::gpio::{Input, InputConfig, Pull};
use esp_hal::main;
use esp_hal::pcnt::Pcnt;
use esp_hal::time::{Duration, Instant};
use esp_hal::timer::timg::TimerGroup;
use esp_println as _;
use esp_radio::ble::controller::BleConnector;
use firmware::Board;
use firmware::quadrature::QuadratureDecoder;

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    error!("{}", panic_info);
    loop {}
}

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[main]
fn main() -> ! {
    // generator version: 1.3.0
    // generator parameters: --chip esp32c6 -o defmt -o nightly-2026-01-22-x86_64-unknown-linux-gnu -o alloc -o unstable-hal -o wifi -o ble-bleps

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 65536);
    // COEX needs more RAM - so we've added some more
    esp_alloc::heap_allocator!(size: 64 * 1024);

    let mut board = Board::new();

    let quad_pull = InputConfig::default().with_pull(Pull::Up);
    let encoder = QuadratureDecoder::new(
        Pcnt::new(
            board
                .remaining
                .pcnt,
        )
        .unit0,
        Input::new(board.d0, quad_pull),
        Input::new(board.d1, quad_pull),
    );

    let timg0 = TimerGroup::new(
        board
            .remaining
            .timg0,
    );
    let sw_interrupt = esp_hal::interrupt::software::SoftwareInterruptControl::new(
        board
            .remaining
            .sw_interrupt,
    );
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);
    let (mut _wifi_controller, _interfaces) = esp_radio::wifi::new(
        board
            .remaining
            .wifi,
        Default::default(),
    )
    .expect("Failed to initialize Wi-Fi controller");
    let _connector = BleConnector::new(
        board
            .remaining
            .bt,
        Default::default(),
    );

    loop {
        board
            .user_led
            .toggle();
        info!("encoder count: {}", encoder.count());
        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(500) {}
    }

    // for inspiration, have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.1.0/examples
}
