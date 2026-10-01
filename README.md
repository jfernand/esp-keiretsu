# ESP-Keiretsu

[![CI](https://github.com/jfernand/esp-keiretsu/actions/workflows/ci.yml/badge.svg)](https://github.com/jfernand/esp-keiretsu/actions/workflows/ci.yml)

Bare-metal Rust firmware for the ESP32-C6 (RISC-V) driving an electronic leadscrew on a lathe:
a stepper on the carriage follows spindle rotation, read from a quadrature encoder, at a
runtime-configurable ratio. The controller is commandable over BLE or TCP using a small
RESP-style protocol, with a host CLI to drive it.

## Workspace

| Crate | Path | Description |
|---|---|---|
| `firmware` | [`firmware/`](firmware) | `no_std` firmware: PCNT quadrature decoder, MCPWM step/dir driver, and a sans-io leadscrew command engine |
| `esp-xy-client` | [`client/`](client) | Host CLI for the controller, over BLE (btleplug) or TCP |
| `resp-over-x` | [`resp/`](resp) | Tiny RESP-inspired command/reply framing for any byte stream; `no_std`, no-alloc by default |
| `xiao-esp32c6-bsp` | [`xiao-esp32c6-bsp/`](xiao-esp32c6-bsp) | Board support for the Seeed Studio XIAO ESP32-C6 |
| `esp32c6-devkitm1-bsp` | [`devkitm1-bsp/`](devkitm1-bsp) | Board support for ESP32-C6-WROOM-1 boards based on the DevKitM-1 design |

### Firmware binaries

- `leadscrew`: electronic leadscrew controlled over a BLE Nordic UART Service. Any generic BLE
  terminal app works, as does the client.
- `leadscrew_tcp`: same control loop, controlled over TCP on the device's WiFi station address.
- `encoder`: quadrature encoder bring-up.
- `esp-xy`: the original scaffold binary (encoder + BLE).

## Building

The toolchain (nightly, with the `riscv32imac-unknown-none-elf` target and `rust-src`) is
pinned in [`rust-toolchain.toml`](rust-toolchain.toml) and installs automatically via rustup.

Firmware is built from `firmware/`, which holds the `.cargo/config.toml` that sets the target,
`build-std`, and the `espflash` runner:

```sh
cd firmware
cargo build --release --bin leadscrew
cargo run --release --bin leadscrew   # flash and monitor via espflash (defmt logs)
```

The TCP binary reads WiFi credentials at compile time:

```sh
WIFI_SSID=... WIFI_PASSWORD=... cargo run --release --bin leadscrew_tcp
```

The host client builds from the workspace root (on Linux it needs `libdbus-1-dev` and
`pkg-config` for BLE):

```sh
cargo run -p esp-xy-client -- scan
cargo run -p esp-xy-client -- status                       # first BLE device found
cargo run -p esp-xy-client -- --host 192.168.1.50 monitor  # over TCP (default port 6379)
```

Client commands: `scan`, `status`, `mode <FEED|THREAD|JOG>`, `ratio <µm>`, `dir <FWD|REV>`,
`enable`, `disable`, `clear`, `monitor`.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
