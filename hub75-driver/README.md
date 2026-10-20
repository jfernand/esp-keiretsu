# esp32c6-hub75-driver

Driver for two daisy-chained 64x64 HUB75E RGB LED matrix panels on the ESP32-C6, built on the
[PARL_IO](https://docs.espressif.com/projects/esp32-c6/en/latest/esp32c6/api-reference/peripherals/parl_io.html)
peripheral: all 13 HUB75 control signals (6 color bits, 5 row-address bits, latch, output-enable)
ride a single 16-bit PARL_IO bus word, clocked out by PARL_IO's own clock pad rather than
bit-banged GPIOs.

Datasheets/schematics for the panel and chip live under [`reference/`](../reference) at the
workspace root -- **this crate assumes 1/32 scan (5 address lines); confirm that against your
actual panel before trusting the row-pairing math in `encode_frame`.**

## Status

Compiles and clippy-clean against `esp-hal` 1.1.2 on the `riscv32imac-unknown-none-elf` target
(verified 2026-10-20), but **not yet bench-tested against real hardware**. Treat the pin
timing, chunk sizing, and chain shift-order as a documented first cut, not a validated design.

## Why this streams in chunks instead of running freely

The obvious design -- precompute one whole frame's bitstream (BCM-expanded, with address/latch/OE
baked in) and point a circular GDMA descriptor ring at it so the DMA engine loops forever with
zero CPU involvement -- doesn't work on this peripheral. `esp-hal`'s own `DmaLoopBuf` proves the
*mechanism* (a descriptor that points back to itself, never signaling end-of-frame) is real, but
PARL_IO's TX core has its own hardware frame-length register (`PARL_IO_TX_BYTELEN`, ESP32-C6 TRM
ch. 38) that ends the bus transaction after that many bytes regardless of how much more data the
DMA descriptors have queued -- `esp-hal` enforces this as a 32736-byte cap per `write()` call,
well under one full frame.

So scanout here is a software loop of chunked DMA writes (`Hub75Driver::send_chunk`), not a
single free-running transfer. It's still cheap -- DMA moves all the bytes; the CPU only re-arms
a new transfer once per chunk -- and in `Async` mode `ParlIoTxTransfer::wait_for_done` is
interrupt-driven, so the loop can run as a low-priority embassy task instead of busy-polling.

## Frame size

At the default 4-bit-per-channel color depth (4096 colors), one fully bit-angle-modulated frame
is 17,248 words / 34,496 bytes -- two chunks under the 32736-byte cap. Raising `COLOR_DEPTH`
trades SRAM and chunk count for color resolution; see `frame_words()` for the formula.

## What's not done yet

- `Hub75Driver` wiring (pin assignment, `ParlIo`/DMA setup, chunked `write()` loop) is written
  against the real `esp-hal` 1.1.2 API and compiles, but has not been run against a panel.
- No gamma/brightness correction before bit-slicing in `encode_frame` -- straight binary weights,
  which will look washed out at low brightness.
- The shift/display phases are sequential, not overlapped, so refresh rate is lower than an
  optimized driver could achieve. Worth revisiting once the basic version is confirmed working.
- No double-buffering yet: redraw into the same `FrameBuffer`/encoded bitstream in place; minor
  tearing is possible if you rebuild mid-scan.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
