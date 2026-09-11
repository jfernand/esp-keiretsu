#![no_std]

//! Board support package for generic ESP32-C6-WROOM-1 dev boards built on Espressif's
//! official ESP32-C6-DevKitM-1 reference design (the layout most cheap Amazon/AliExpress
//! ESP32-C6-WROOM-1-N4 boards clone) -- covers the two headers (J1/J3) in Espressif's user
//! guide: <https://docs.espressif.com/projects/esp-dev-kits/en/latest/esp32c6/esp32-c6-devkitm-1/user_guide.html>
//!
//! Fixed-function pins (board-defined behavior, wrapped below):
//! - GPIO9: BOOT button (also the boot-mode strapping pin -- see [`BootButton`])
//! - GPIO8: addressable RGB LED data line -- left as a raw pin ([`Board::rgb_led_data`])
//!   rather than wrapped, since driving it needs a proper WS2812-style bit-timed driver, not
//!   a simple digital write
//!
//! Header pins (exposed as raw peripherals; the app configures them as GPIO/ADC/UART/I2C/JTAG):
//! - GPIO0-GPIO7: J1 header, general purpose / ADC1 channels 0-5 on GPIO0-GPIO5; GPIO6/GPIO7
//!   double as I2C SDA/SCL and, with GPIO4/GPIO5, as JTAG (MTMS/MTDI/MTCK/MTDO)
//! - GPIO14: J1 header, general purpose
//! - GPIO15: J3 header, general purpose (also a strapping pin -- see chip datasheet before
//!   driving it at boot)
//! - GPIO16/GPIO17: J3 header, wired to the onboard USB-to-UART bridge as TX/RX
//! - GPIO18-GPIO23: J3 header, general purpose
//!
//! Not exposed here because they aren't available as controllable GPIOs on this board:
//! RST (hardware EN, not a GPIO), GPIO12/GPIO13 (wired to the native USB D-/D+ lines), 3V3/5V/G
//! (power rails).

use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig};
use esp_hal::peripherals;

/// Raw GPIO peripherals for every pin exposed on the board's two headers.
///
/// Pull these off the `Peripherals` returned by `esp_hal::init` and pass them to
/// [`Board::new`]. Every other peripheral (timers, radio, ...) is left untouched for the
/// application to use directly.
pub struct BoardPeripherals<'d> {
    // Fixed-function pins.
    pub boot_button: peripherals::GPIO9<'d>,
    pub rgb_led_data: peripherals::GPIO8<'d>,

    // J1 header (besides BOOT/RGB above).
    pub io0: peripherals::GPIO0<'d>,
    pub io1: peripherals::GPIO1<'d>,
    pub io2: peripherals::GPIO2<'d>,
    pub io3: peripherals::GPIO3<'d>,
    pub io4: peripherals::GPIO4<'d>,
    pub io5: peripherals::GPIO5<'d>,
    pub io6: peripherals::GPIO6<'d>,
    pub io7: peripherals::GPIO7<'d>,
    pub io14: peripherals::GPIO14<'d>,

    // J3 header.
    pub uart0_tx: peripherals::GPIO16<'d>,
    pub uart0_rx: peripherals::GPIO17<'d>,
    pub io18: peripherals::GPIO18<'d>,
    pub io19: peripherals::GPIO19<'d>,
    pub io20: peripherals::GPIO20<'d>,
    pub io21: peripherals::GPIO21<'d>,
    pub io22: peripherals::GPIO22<'d>,
    pub io23: peripherals::GPIO23<'d>,
    pub io15: peripherals::GPIO15<'d>,

    /// Everything else: timers, the radio, and any other peripheral the application still
    /// needs to set up itself.
    pub remaining: RemainingPeripherals<'d>,
}

/// BOOT button (GPIO9), read as active low.
///
/// This is also the boot-mode strapping pin (held low at reset to enter the ROM download
/// mode), so treat it as read-only input during normal operation -- never drive it from
/// software.
pub struct BootButton<'d>(Input<'d>);

impl<'d> BootButton<'d> {
    pub fn is_pressed(&self) -> bool {
        self.0
            .is_low()
    }
}

/// Owns every exposed board pin and gives the fixed-function ones board-correct names instead
/// of raw GPIO numbers.
pub struct Board<'d> {
    pub boot_button: BootButton<'d>,
    /// Addressable RGB LED data line -- drive with a WS2812-style driver, not a plain digital
    /// write.
    pub rgb_led_data: peripherals::GPIO8<'d>,

    pub io0: peripherals::GPIO0<'d>,
    pub io1: peripherals::GPIO1<'d>,
    pub io2: peripherals::GPIO2<'d>,
    pub io3: peripherals::GPIO3<'d>,
    pub io4: peripherals::GPIO4<'d>,
    pub io5: peripherals::GPIO5<'d>,
    pub io6: peripherals::GPIO6<'d>,
    pub io7: peripherals::GPIO7<'d>,
    pub io14: peripherals::GPIO14<'d>,

    /// UART0 TX line, wired to the onboard USB-to-UART bridge.
    pub uart0_tx: peripherals::GPIO16<'d>,
    /// UART0 RX line, wired to the onboard USB-to-UART bridge.
    pub uart0_rx: peripherals::GPIO17<'d>,
    pub io18: peripherals::GPIO18<'d>,
    pub io19: peripherals::GPIO19<'d>,
    pub io20: peripherals::GPIO20<'d>,
    pub io21: peripherals::GPIO21<'d>,
    pub io22: peripherals::GPIO22<'d>,
    pub io23: peripherals::GPIO23<'d>,
    pub io15: peripherals::GPIO15<'d>,

    /// Everything [`BoardPeripherals`] didn't claim a name for: timers, the radio, and
    /// anything else the application still needs to set up itself.
    pub remaining: RemainingPeripherals<'d>,
}

impl Board<'static> {
    /// Initializes the board using peripherals obtained by [`esp_hal::init`].
    ///
    /// This is the app's hardware entry point: it must be called at most once
    /// (`esp_hal::init` panics on a second call), and nothing else in the app should call
    /// `esp_hal::init` itself.
    pub fn new() -> Self {
        Self::from_peripherals(BoardPeripherals::default())
    }
}

impl Default for Board<'static> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'d> Board<'d> {
    /// Initializes the board's fixed-function pins from already-obtained peripherals, and
    /// hands back the rest untouched.
    pub fn from_peripherals(pins: BoardPeripherals<'d>) -> Self {
        let boot_button = BootButton(Input::new(pins.boot_button, InputConfig::default()));

        Self {
            boot_button,
            rgb_led_data: pins.rgb_led_data,
            io0: pins.io0,
            io1: pins.io1,
            io2: pins.io2,
            io3: pins.io3,
            io4: pins.io4,
            io5: pins.io5,
            io6: pins.io6,
            io7: pins.io7,
            io14: pins.io14,
            uart0_tx: pins.uart0_tx,
            uart0_rx: pins.uart0_rx,
            io18: pins.io18,
            io19: pins.io19,
            io20: pins.io20,
            io21: pins.io21,
            io22: pins.io22,
            io23: pins.io23,
            io15: pins.io15,
            remaining: pins.remaining,
        }
    }
}

/// Peripherals [`BoardPeripherals`] doesn't assign a board-specific name to: timers, the
/// radio, and anything else the application still needs to set up itself.
pub struct RemainingPeripherals<'d> {
    pub timg0: peripherals::TIMG0<'d>,
    pub sw_interrupt: peripherals::SW_INTERRUPT<'d>,
    pub wifi: peripherals::WIFI<'d>,
    pub bt: peripherals::BT<'d>,
    pub pcnt: peripherals::PCNT<'d>,
    pub mcpwm0: peripherals::MCPWM0<'d>,
}

impl Default for BoardPeripherals<'static> {
    /// Calls [`esp_hal::init`] and sorts the result into board pins plus
    /// [`RemainingPeripherals`].
    ///
    /// Like `esp_hal::init`, this must be called at most once.
    fn default() -> Self {
        let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
        let peripherals = esp_hal::init(config);

        Self {
            boot_button: peripherals.GPIO9,
            rgb_led_data: peripherals.GPIO8,
            io0: peripherals.GPIO0,
            io1: peripherals.GPIO1,
            io2: peripherals.GPIO2,
            io3: peripherals.GPIO3,
            io4: peripherals.GPIO4,
            io5: peripherals.GPIO5,
            io6: peripherals.GPIO6,
            io7: peripherals.GPIO7,
            io14: peripherals.GPIO14,
            uart0_tx: peripherals.GPIO16,
            uart0_rx: peripherals.GPIO17,
            io18: peripherals.GPIO18,
            io19: peripherals.GPIO19,
            io20: peripherals.GPIO20,
            io21: peripherals.GPIO21,
            io22: peripherals.GPIO22,
            io23: peripherals.GPIO23,
            io15: peripherals.GPIO15,
            remaining: RemainingPeripherals {
                timg0: peripherals.TIMG0,
                sw_interrupt: peripherals.SW_INTERRUPT,
                wifi: peripherals.WIFI,
                bt: peripherals.BT,
                pcnt: peripherals.PCNT,
                mcpwm0: peripherals.MCPWM0,
            },
        }
    }
}
