#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use defmt::{error, info};
use esp_hal::gpio::{Input, InputConfig, Pull};
use esp_hal::main;
use esp_hal::pcnt::Pcnt;
use esp_println as _;
use firmware::Board;
use firmware::quadrature::QuadratureDecoder;

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    error!("{}", panic_info);
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let board = Board::new();

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

    let mut last = encoder.count();
    info!("encoder count: {}", last);

    loop {
        let current = encoder.count();
        if current != last {
            info!("encoder count: {}", current);
            last = current;
        }
    }
}
