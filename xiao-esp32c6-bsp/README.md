# xiao-esp32c6-bsp

Board support package for the [Seeed Studio XIAO ESP32-C6](https://wiki.seeedstudio.com/xiao_esp32c6_getting_started/),
covering every pin in the board's front pinout. Built on [`esp-hal`](https://docs.rs/esp-hal).

`Board::new()` claims the fixed-function pins with board-correct names and polarity instead of
raw GPIO numbers, and hands back everything else (header pins, timers, radio, ...) untouched for
the application to configure itself:

- **GPIO15** — onboard user LED (`Board::user_led`)
- **GPIO3** — RF power switch, active low (`Board::rf_switch`)
- **GPIO14** — antenna switch, active low (`Board::antenna_switch`)
- **GPIO9** — BOOT button / boot-mode strapping pin, read-only (`Board::boot_button`)
- Header pins D0–D10 (I2C, UART0, SPI, general-purpose/ADC) — exposed as raw peripherals for the
  application to configure as it needs

```rust,ignore
use xiao_esp32c6_bsp::Board;

let mut board = Board::new();
board.user_led.on();
```

`Board::new()` calls `esp_hal::init` and must be called at most once, matching `esp_hal::init`'s
own contract.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
