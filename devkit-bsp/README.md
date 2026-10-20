# esp32c6-devkit-bsp

Board support package for Espressif's official
[ESP32-C6-DevKitC-1](https://docs.espressif.com/projects/esp-dev-kits/en/latest/esp32c6/esp32-c6-devkitc-1/user_guide.html)
and [ESP32-C6-DevKitM-1](https://docs.espressif.com/projects/esp-dev-kits/en/latest/esp32c6/esp32-c6-devkitm-1/user_guide.html)
reference designs -- also covers the generic ESP32-C6-WROOM-1 dev boards on Amazon/AliExpress
that clone one of these header layouts. Built on [`esp-hal`](https://docs.rs/esp-hal).

Downloaded copies of both boards' user guides, schematics, and module datasheets live under
[`reference/`](../reference) at the workspace root.

Enable exactly one Cargo feature for the board you have -- `devkitc1` or `devkitm1` -- the crate
fails to compile if neither or both are enabled, so there's no silent wrong-board default:

```toml
esp32c6-devkit-bsp = { version = "0.1", features = ["devkitc1"] }
```

The two boards share the same pin assignments except that DevKitC-1 additionally breaks out
**GPIO10/GPIO11** on J1 (`Board::io10`/`Board::io11`, only present with `devkitc1`); DevKitM-1
doesn't wire them to a header at all.

`Board::new()` claims the board's fixed-function pins with board-correct names, and hands back
every header pin (J1/J3) as a raw peripheral for the application to configure as GPIO, ADC,
UART, I2C, or JTAG as needed:

- **GPIO9** -- BOOT button / boot-mode strapping pin, read-only (`Board::boot_button`)
- **GPIO8** -- addressable RGB LED data line (`Board::rgb_led_data`), left as a raw pin since
  driving it needs a WS2812-style bit-timed driver, not a plain digital write
- **GPIO16/GPIO17** -- UART0 TX/RX, wired to the onboard USB-to-UART bridge
  (`Board::uart0_tx`/`Board::uart0_rx`)
- Every other header pin (GPIO0-GPIO7, GPIO14, GPIO15, GPIO18-GPIO23, plus GPIO10/GPIO11 on
  DevKitC-1) as `Board::io<N>`

Not exposed: RST (hardware EN, not a GPIO) and GPIO12/GPIO13 (wired to the native USB D-/D+
lines).

```rust,ignore
use esp32c6_devkit_bsp::Board;

let board = Board::new();
let pressed = board.boot_button.is_pressed();
```

`Board::new()` calls `esp_hal::init` and must be called at most once, matching `esp_hal::init`'s
own contract.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
