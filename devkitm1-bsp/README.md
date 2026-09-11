# esp32c6-devkitm1-bsp

Board support package for generic ESP32-C6-WROOM-1 dev boards built on Espressif's official
[ESP32-C6-DevKitM-1](https://docs.espressif.com/projects/esp-dev-kits/en/latest/esp32c6/esp32-c6-devkitm-1/user_guide.html)
reference design -- the layout most cheap ESP32-C6-WROOM-1-N4 boards on Amazon/AliExpress
clone. Built on [`esp-hal`](https://docs.rs/esp-hal).

`Board::new()` claims the board's fixed-function pins with board-correct names, and hands back
every header pin (J1/J3) as a raw peripheral for the application to configure as GPIO, ADC,
UART, I2C, or JTAG as needed:

- **GPIO9** -- BOOT button / boot-mode strapping pin, read-only (`Board::boot_button`)
- **GPIO8** -- addressable RGB LED data line (`Board::rgb_led_data`), left as a raw pin since
  driving it needs a WS2812-style bit-timed driver, not a plain digital write
- **GPIO16/GPIO17** -- UART0 TX/RX, wired to the onboard USB-to-UART bridge
  (`Board::uart0_tx`/`Board::uart0_rx`)
- Every other header pin (GPIO0-GPIO7, GPIO14, GPIO15, GPIO18-GPIO23) as `Board::io<N>`

Not exposed: RST (hardware EN, not a GPIO) and GPIO12/GPIO13 (wired to the native USB D-/D+
lines).

```rust,ignore
use esp32c6_devkitm1_bsp::Board;

let board = Board::new();
let pressed = board.boot_button.is_pressed();
```

`Board::new()` calls `esp_hal::init` and must be called at most once, matching `esp_hal::init`'s
own contract.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
