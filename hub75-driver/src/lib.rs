#![no_std]

//! HUB75 LED matrix driver for ESP32-C6, built on the PARL_IO peripheral.
//!
//! Assumes two daisy-chained 64x64 HUB75E (1/32-scan, 5 address lines) panels
//! forming a single 128x64 logical canvas with one set of control signals --
//! **verify the scan type against your panel's actual datasheet** before
//! trusting the row-pairing math below; a 1/16-scan panel needs 4 address
//! lines and pairs rows differently.
//!
//! ## Why this isn't a "set it and forget it" free-running driver
//!
//! The obvious design -- precompute one big bitstream for a whole frame
//! (including address/latch/OE) and point a circular GDMA descriptor ring at
//! it so the DMA engine loops forever with zero CPU involvement -- does not
//! work here. `esp-hal`'s [`esp_hal::dma::buffers::DmaLoopBuf`] proves the
//! *mechanism* (a descriptor whose `next` points back to itself, `suc_eof`
//! never set) works, but PARL_IO's TX core has its own hardware frame-length
//! register (`PARL_IO_TX_BYTELEN`, TRM chapter 38) that is independent of the
//! DMA descriptor chain -- it ends the bus transaction after that many bytes
//! regardless of how much more data the descriptors have queued up. The
//! public `esp-hal` API enforces this as a hard cap of 32736 bytes per
//! [`esp_hal::parl_io::ParlIoTx::write`] call
//! (`esp_hal::parl_io::Error::MaxDmaTransferSizeExceeded`), well under one
//! full bit-angle-modulated frame (tens of KB).
//!
//! So scanout here is a **software-driven loop of chunked DMA writes**, not a
//! single free-running transfer: split the frame into chunks under that cap,
//! `write()` each one, and move to the next as soon as it completes --
//! cheap (the DMA still does all the byte-shoving; the CPU only re-arms a new
//! transfer every chunk), but not literally zero-CPU. In `Async` mode
//! `ParlIoTxTransfer::wait_for_done` is interrupt-driven, so this loop can run
//! as a low-priority embassy task rather than busy-polling.

use esp_hal::gpio::interconnect::PeripheralOutput;
use esp_hal::gpio::NoPin;
use esp_hal::parl_io::{
    BitPackOrder,
    ClkOutPin,
    ConfigError,
    Error as ParlIoError,
    ParlIo,
    ParlIoTx,
    TxConfig,
    TxSixteenBits,
};
use esp_hal::peripherals::{DMA_CH0, PARL_IO};
use esp_hal::time::Rate;
use esp_hal::Blocking;
use esp_hal::dma::{DmaError, DmaTxBuf};

/// Columns per physical panel.
pub const PANEL_COLS: usize = 64;
/// Panels daisy-chained into one logical row -- see the module docs for how
/// shift order depends on this.
pub const PANELS_CHAINED: usize = 2;
/// Logical canvas width (both panels side by side).
pub const COLS: usize = PANEL_COLS * PANELS_CHAINED;
/// Logical canvas height (unaffected by chaining -- chaining only extends
/// width).
pub const ROWS: usize = 64;
/// Row-pairs addressed per scan line on a 1/32-scan panel (5 address lines).
pub const SCAN_ROWS: usize = ROWS / 2;
/// Bit-angle-modulation planes per color channel. 4 bits/channel (4096
/// colors) keeps the frame buffer small and the refresh rate high; raise it
/// if `esp-keiretsu`'s SRAM budget allows and banding is visible.
pub const COLOR_DEPTH: u32 = 4;

/// Bit positions within each 16-bit PARL_IO bus word. D13-D15 are unused
/// (wired to [`NoPin`] in [`Hub75Driver::new`]).
mod bus {
    pub const R1: u16 = 1 << 0;
    pub const G1: u16 = 1 << 1;
    pub const B1: u16 = 1 << 2;
    pub const R2: u16 = 1 << 3;
    pub const G2: u16 = 1 << 4;
    pub const B2: u16 = 1 << 5;
    pub const ADDR_SHIFT: u32 = 6; // 5 bits: A=6, B=7, C=8, D=9, E=10
    pub const LAT: u16 = 1 << 11;
    pub const OE: u16 = 1 << 12;
}

/// How many extra words (beyond the base 1) bit-plane `plane` is held
/// displayed for, binary-weighted so plane `b` is `2^b` times longer than
/// plane 0.
const fn plane_display_words(plane: u32) -> usize {
    1usize << plane
}

const SHIFT_WORDS: usize = COLS;
const ROW_OVERHEAD_WORDS: usize = 3; // addr-settle + 2 latch-pulse words

/// Total words in one fully bit-angle-modulated frame.
pub const fn frame_words() -> usize {
    let mut total = 0usize;
    let mut plane = 0u32;
    while plane < COLOR_DEPTH {
        total += SCAN_ROWS * (SHIFT_WORDS + ROW_OVERHEAD_WORDS + plane_display_words(plane));
        plane += 1;
    }
    total
}

pub const FRAME_WORDS: usize = frame_words();
pub const FRAME_BYTES: usize = FRAME_WORDS * 2;

/// Logical 128x64 RGB pixel canvas (both chained panels).
pub struct FrameBuffer {
    pixels: [[u8; 3]; COLS * ROWS],
}

impl FrameBuffer {
    pub const fn new() -> Self {
        Self {
            pixels: [[0; 3]; COLS * ROWS],
        }
    }

    pub fn clear(&mut self) {
        self.pixels = [[0; 3]; COLS * ROWS];
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, rgb: [u8; 3]) {
        if x < COLS && y < ROWS {
            self.pixels[y * COLS + x] = rgb;
        }
    }

    fn pixel(&self, x: usize, y: usize) -> [u8; 3] {
        self.pixels[y * COLS + x]
    }
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Daisy-chain shift order: panel 2 sits behind panel 1 in the chain, so its
/// pixels must be shifted in first (see the module docs' chain-order note).
/// Flip this (`COLS - 1 - s` either side) if the two halves come out
/// column-swapped on your wiring.
fn shift_index_to_x(s: usize) -> usize {
    if s < PANEL_COLS {
        PANEL_COLS + s // far panel (panel 2) columns first
    } else {
        s - PANEL_COLS // near panel (panel 1) columns last
    }
}

/// Encodes `fb` into one full bit-angle-modulated PARL_IO bitstream.
///
/// This is a first-cut, unoptimized (non-overlapped shift/display) encoder:
/// no gamma correction is applied before bit-slicing, and shifting always
/// waits for the previous row's display dwell to finish rather than
/// overlapping the two. Both are reasonable places to optimize once this is
/// bench-verified against real hardware.
pub fn encode_frame(fb: &FrameBuffer, out: &mut [u16; FRAME_WORDS]) {
    let mut w = 0usize;

    for addr in 0..SCAN_ROWS {
        let addr_bits = (addr as u16) << bus::ADDR_SHIFT;

        for plane in 0..COLOR_DEPTH {
            let bit = 1u8 << plane;

            // Shift phase: blanked (OE=1), push this row's color bits for the
            // upcoming latch. Address lines carry the *previous* value here --
            // they only need to be correct once we're about to latch below.
            for s in 0..COLS {
                let x = shift_index_to_x(s);
                let top = fb.pixel(x, addr);
                let bottom = fb.pixel(x, addr + SCAN_ROWS);

                let mut word = bus::OE | addr_bits;
                if top[0] & bit != 0 {
                    word |= bus::R1;
                }
                if top[1] & bit != 0 {
                    word |= bus::G1;
                }
                if top[2] & bit != 0 {
                    word |= bus::B1;
                }
                if bottom[0] & bit != 0 {
                    word |= bus::R2;
                }
                if bottom[1] & bit != 0 {
                    word |= bus::G2;
                }
                if bottom[2] & bit != 0 {
                    word |= bus::B2;
                }
                out[w] = word;
                w += 1;
            }

            // Address settle, then a latch pulse, both still blanked.
            out[w] = bus::OE | addr_bits;
            w += 1;
            out[w] = bus::OE | bus::LAT | addr_bits;
            w += 1;
            out[w] = bus::OE | addr_bits;
            w += 1;

            // Display dwell: unblanked (OE=0), held for this plane's weight.
            for _ in 0..plane_display_words(plane) {
                out[w] = addr_bits;
                w += 1;
            }
        }
    }

    debug_assert_eq!(w, FRAME_WORDS);
}

/// Errors from [`Hub75Driver`].
#[derive(Debug)]
pub enum Hub75Error {
    Config(ConfigError),
    ParlIo(ParlIoError),
    Dma(DmaError),
}

impl From<ConfigError> for Hub75Error {
    fn from(value: ConfigError) -> Self {
        Self::Config(value)
    }
}

impl From<ParlIoError> for Hub75Error {
    fn from(value: ParlIoError) -> Self {
        Self::ParlIo(value)
    }
}

impl From<DmaError> for Hub75Error {
    fn from(value: DmaError) -> Self {
        Self::Dma(value)
    }
}

/// Drives the 13 HUB75 control signals over PARL_IO's 16-bit TX bus (3 lines
/// unused, tied to [`NoPin`]).
///
/// Owns the `ParlIoTx` between chunk writes (`Option`-dance because
/// `ParlIoTx::write` consumes `self` and only hands it back once the transfer
/// completes).
pub struct Hub75Driver<'d> {
    tx: Option<ParlIoTx<'d, Blocking>>,
}

impl<'d> Hub75Driver<'d> {
    /// `bus_clock` is the PARL_IO pixel clock (max 40 MHz per the ESP32-C6
    /// TRM) -- start conservative (e.g. 10-20 MHz) and raise it once the
    /// panel is confirmed to track reliably.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        parl_io: PARL_IO<'d>,
        dma_channel: DMA_CH0<'d>,
        r1: impl PeripheralOutput<'d>,
        g1: impl PeripheralOutput<'d>,
        b1: impl PeripheralOutput<'d>,
        r2: impl PeripheralOutput<'d>,
        g2: impl PeripheralOutput<'d>,
        b2: impl PeripheralOutput<'d>,
        addr_a: impl PeripheralOutput<'d>,
        addr_b: impl PeripheralOutput<'d>,
        addr_c: impl PeripheralOutput<'d>,
        addr_d: impl PeripheralOutput<'d>,
        addr_e: impl PeripheralOutput<'d>,
        lat: impl PeripheralOutput<'d>,
        oe: impl PeripheralOutput<'d>,
        clk: impl PeripheralOutput<'d>,
        bus_clock: Rate,
    ) -> Result<Self, Hub75Error> {
        let pins = TxSixteenBits::new(
            r1, g1, b1, r2, g2, b2, addr_a, addr_b, addr_c, addr_d, addr_e, lat, oe, NoPin, NoPin,
            NoPin,
        );
        let clk_pin = ClkOutPin::new(clk);

        // Idle (between chunk writes) holds OE high so a gap between writes
        // shows as blanked, not a frozen/ghosted last row.
        let config = TxConfig::default()
            .with_frequency(bus_clock)
            .with_bit_order(BitPackOrder::Lsb)
            .with_idle_value(bus::OE);

        let parl_io = ParlIo::new(parl_io, dma_channel)?;
        let tx = parl_io.tx.with_config(pins, clk_pin, config)?;

        Ok(Self { tx: Some(tx) })
    }

    /// Sends one chunk of an [`encode_frame`]d bitstream and blocks until it
    /// completes. `len` must be `<= 32736` bytes (PARL_IO's TX_BYTELEN cap --
    /// see the module docs) and `<= buf`'s length.
    ///
    /// Call this repeatedly over successive slices of the frame to stream a
    /// whole frame, then repeat the whole sequence to keep the panel lit --
    /// there is no free-running mode to fall back to between calls.
    pub fn send_chunk(&mut self, len: usize, buf: DmaTxBuf) -> Result<DmaTxBuf, Hub75Error> {
        let tx = self
            .tx
            .take()
            .expect("Hub75Driver::tx is only None inside this method");

        match tx.write(len, buf) {
            Ok(transfer) => {
                let (result, tx, buf) = transfer.wait();
                self.tx = Some(tx);
                result.map(|()| buf).map_err(Hub75Error::from)
            }
            Err((err, tx, _buf)) => {
                self.tx = Some(tx);
                Err(Hub75Error::from(err))
            }
        }
    }
}
