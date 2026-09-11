#![no_std]

//! Board support package, covering every pin in `XIAO_ESP32-C6_front_pinout.png`.
//!
//! Fixed-function pins (board-defined behavior, wrapped below):
//! - GPIO15: onboard user LED
//! - GPIO3:  RF power switch, active low (low = RF enabled)
//! - GPIO14: antenna switch, active low (low = internal/onboard antenna, high = external u.FL)
//! - GPIO9:  BOOT button (also the boot-mode strapping pin — see [`BootButton`])
//!
//! Header pins (exposed as raw peripherals; the app configures them as GPIO/ADC/UART/SPI):
//! - GPIO22 / D4: I2C SDA
//! - GPIO23 / D5: I2C SCL (inferred from the pinout diagram; only SDA was specified)
//! - GPIO16 / D6: UART0 TX
//! - GPIO17 / D7: UART0 RX
//! - GPIO18 / D10: SPI MOSI
//! - GPIO20 / D9:  SPI MISO
//! - GPIO19 / D8:  SPI SCK
//! - GPIO0  / D0: general purpose, ADC1 channel 0, RTC/LP GPIO0
//! - GPIO1  / D1: general purpose, ADC1 channel 1, RTC/LP GPIO1
//! - GPIO2  / D2: general purpose, ADC1 channel 2, RTC/LP GPIO2
//! - GPIO21 / D3: general purpose
//!
//! Not exposed here because they aren't wired to the MCU as controllable GPIOs:
//! RESET/CHIP_EN (hardware reset), CHARGE_LED (driven by the battery-charger IC),
//! VBUS/GND/3V3-OUT (power rails).

use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig};
use esp_hal::peripherals;

/// Raw GPIO peripherals for every pin on the board.
///
/// Pull these off the `Peripherals` returned by `esp_hal::init` and pass them to
/// [`Board::new`]. Every other peripheral (timers, radio, ...) is left untouched
/// for the application to use directly.
pub struct BoardPeripherals<'d> {
    // Fixed-function pins.
    pub user_led: peripherals::GPIO15<'d>,
    pub rf_enable: peripherals::GPIO3<'d>,
    pub antenna_switch: peripherals::GPIO14<'d>,
    pub boot_button: peripherals::GPIO9<'d>,

    // I2C header pins (D4 / D5).
    pub i2c_sda: peripherals::GPIO22<'d>,
    pub i2c_scl: peripherals::GPIO23<'d>,

    // UART0 header pins (D6 / D7).
    pub uart0_tx: peripherals::GPIO16<'d>,
    pub uart0_rx: peripherals::GPIO17<'d>,

    // SPI header pins (D10 / D9 / D8).
    pub spi_mosi: peripherals::GPIO18<'d>,
    pub spi_miso: peripherals::GPIO20<'d>,
    pub spi_sck: peripherals::GPIO19<'d>,

    // General-purpose / ADC header pins (D0 - D3).
    pub d0: peripherals::GPIO0<'d>,
    pub d1: peripherals::GPIO1<'d>,
    pub d2: peripherals::GPIO2<'d>,
    pub d3: peripherals::GPIO21<'d>,

    /// Everything else: timers, the radio, and any other peripheral the
    /// application still needs to set up itself.
    pub remaining: RemainingPeripherals<'d>,
}

/// Which antenna the RF front end is routed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Antenna {
    /// Onboard PCB antenna.
    Internal,
    /// External antenna via the u.FL connector.
    External,
}

/// Onboard user LED (GPIO15).
///
/// Assumed active-low, matching other XIAO boards; if it turns out to be wired
/// active-high on this unit, swap the levels in `on`/`off` below.
pub struct UserLed<'d>(Output<'d>);

impl<'d> UserLed<'d> {
    pub fn on(&mut self) {
        self.0
            .set_low();
    }

    pub fn off(&mut self) {
        self.0
            .set_high();
    }

    pub fn toggle(&mut self) {
        self.0
            .toggle();
    }
}

/// RF front-end power switch (GPIO3, active low).
pub struct RfSwitch<'d>(Output<'d>);

impl<'d> RfSwitch<'d> {
    pub fn enable(&mut self) {
        self.0
            .set_low();
    }

    pub fn disable(&mut self) {
        self.0
            .set_high();
    }
}

/// Antenna switch (GPIO14, low = internal, high = external).
pub struct AntennaSwitch<'d>(Output<'d>);

impl<'d> AntennaSwitch<'d> {
    pub fn select(&mut self, antenna: Antenna) {
        match antenna {
            Antenna::Internal => self
                .0
                .set_low(),
            Antenna::External => self
                .0
                .set_high(),
        }
    }
}

/// BOOT button (GPIO9), read as active low.
///
/// This is also the boot-mode strapping pin (held low at reset to enter the
/// ROM download mode), so treat it as read-only input during normal
/// operation — never drive it from software.
pub struct BootButton<'d>(Input<'d>);

impl<'d> BootButton<'d> {
    pub fn is_pressed(&self) -> bool {
        self.0
            .is_low()
    }
}

/// Owns every board pin and exposes the fixed-function ones with
/// board-correct names and polarity instead of raw GPIO numbers.
pub struct Board<'d> {
    pub user_led: UserLed<'d>,
    pub rf_switch: RfSwitch<'d>,
    pub antenna_switch: AntennaSwitch<'d>,
    pub boot_button: BootButton<'d>,

    /// I2C SDA line — pass to your I2C driver of choice.
    pub i2c_sda: peripherals::GPIO22<'d>,
    /// I2C SCL line — pass to your I2C driver of choice.
    pub i2c_scl: peripherals::GPIO23<'d>,

    /// UART0 TX line.
    pub uart0_tx: peripherals::GPIO16<'d>,
    /// UART0 RX line.
    pub uart0_rx: peripherals::GPIO17<'d>,

    /// SPI MOSI line.
    pub spi_mosi: peripherals::GPIO18<'d>,
    /// SPI MISO line.
    pub spi_miso: peripherals::GPIO20<'d>,
    /// SPI SCK line.
    pub spi_sck: peripherals::GPIO19<'d>,

    /// Header pin D0 — general purpose GPIO / ADC1 channel 0.
    pub d0: peripherals::GPIO0<'d>,
    /// Header pin D1 — general purpose GPIO / ADC1 channel 1.
    pub d1: peripherals::GPIO1<'d>,
    /// Header pin D2 — general purpose GPIO / ADC1 channel 2.
    pub d2: peripherals::GPIO2<'d>,
    /// Header pin D3 — general purpose GPIO.
    pub d3: peripherals::GPIO21<'d>,

    /// Everything [`BoardPeripherals`] didn't claim a name for: timers, the
    /// radio, and anything else the application still needs to set up itself.
    pub remaining: RemainingPeripherals<'d>,
}

impl Board<'static> {
    /// Initializes the board using peripherals obtained by [`esp_hal::init`].
    ///
    /// This is the app's hardware entry point: it must be called at most
    /// once (`esp_hal::init` panics on a second call), and nothing else in
    /// the app should call `esp_hal::init` itself.
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
    /// Initializes the board's fixed-function pins from already-obtained
    /// peripherals, and hands back the rest untouched.
    ///
    /// On return, RF is enabled and routed to the internal antenna, and the
    /// LED is off.
    pub fn from_peripherals(pins: BoardPeripherals<'d>) -> Self {
        let user_led = Output::new(pins.user_led, Level::High, OutputConfig::default());

        let mut rf_switch = RfSwitch(Output::new(
            pins.rf_enable,
            Level::High,
            OutputConfig::default(),
        ));
        rf_switch.enable();

        let mut antenna_switch = AntennaSwitch(Output::new(
            pins.antenna_switch,
            Level::High,
            OutputConfig::default(),
        ));
        antenna_switch.select(Antenna::Internal);

        let boot_button = BootButton(Input::new(pins.boot_button, InputConfig::default()));

        Self {
            user_led: UserLed(user_led),
            rf_switch,
            antenna_switch,
            boot_button,
            i2c_sda: pins.i2c_sda,
            i2c_scl: pins.i2c_scl,
            uart0_tx: pins.uart0_tx,
            uart0_rx: pins.uart0_rx,
            spi_mosi: pins.spi_mosi,
            spi_miso: pins.spi_miso,
            spi_sck: pins.spi_sck,
            d0: pins.d0,
            d1: pins.d1,
            d2: pins.d2,
            d3: pins.d3,
            remaining: pins.remaining,
        }
    }
}

/// Peripherals [`BoardPeripherals`] doesn't assign a board-specific name to:
/// timers, the radio, and anything else the application still needs to set
/// up itself.
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
            user_led: peripherals.GPIO15,
            rf_enable: peripherals.GPIO3,
            antenna_switch: peripherals.GPIO14,
            boot_button: peripherals.GPIO9,
            i2c_sda: peripherals.GPIO22,
            i2c_scl: peripherals.GPIO23,
            uart0_tx: peripherals.GPIO16,
            uart0_rx: peripherals.GPIO17,
            spi_mosi: peripherals.GPIO18,
            spi_miso: peripherals.GPIO20,
            spi_sck: peripherals.GPIO19,
            d0: peripherals.GPIO0,
            d1: peripherals.GPIO1,
            d2: peripherals.GPIO2,
            d3: peripherals.GPIO21,
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
