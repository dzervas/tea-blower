#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{self, Input, Pull};
use embassy_rp::peripherals::{PIO0, USB};
use embassy_rp::pio::InterruptHandler;
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_time::Timer;
use gpio::{Level, Output};
use log::*;

use defmt_rtt as _;
use panic_probe as _;

const TIMER_SECS: u64 = 210;
const DEBOUNCE_MS: u64 = 250;
const DOUBLE_PRESS_MS: u64 = 2000;

bind_interrupts!(struct Irqs {
	USBCTRL_IRQ => UsbInterruptHandler<USB>;
	PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

#[embassy_executor::task]
async fn logger_task(driver: Driver<'static, USB>) {
	embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
	let p = embassy_rp::init(Default::default());

	// USB log
	let driver = Driver::new(p.USB, Irqs);
	spawner.spawn(logger_task(driver)).unwrap();

	// Hardware
	let mut blower = Output::new(p.PIN_7, Level::Low);
	let mut button = Input::new(p.PIN_15, Pull::Down);

	// LED
	let mut led_r = Output::new(p.PIN_9, Level::High);
	let mut led_g = Output::new(p.PIN_10, Level::High);
	let mut led_b = Output::new(p.PIN_11, Level::High);

	for _ in 0..3 {
		led_g.set_low();
		Timer::after_millis(250).await;
		led_g.set_high();
		Timer::after_millis(250).await;
	}

	loop {
		button.wait_for_high().await;
		Timer::after_millis(DEBOUNCE_MS).await;

		info!("Button pressed");
		blower.set_high();
		led_b.set_low(); // Show that the button was pressed

		let wait_time = match select(Timer::after_millis(DOUBLE_PRESS_MS), button.wait_for_high()).await {
			// Timed out
			Either::First(_) => {
				info!("Single press");
				led_b.set_low();
				TIMER_SECS
			},
			// Double press
			Either::Second(_) => {
				info!("Double press");
				led_b.set_high();
				led_r.set_low();
				led_g.set_low();
				TIMER_SECS * 2
			},
		};

		Timer::after_millis(250).await;

		let mut clear_all = || {
			blower.set_low();
			led_b.set_high();
			led_r.set_high();
			led_g.set_high();
		};

		// Either the button is pressed again or the timer expires
		match select(Timer::after_secs(wait_time), button.wait_for_high()).await {
			Either::First(_) => {
				clear_all();
				info!("Timer expired");
				for _ in 0..3 {
					led_g.set_low();
					Timer::after_millis(500).await;
					led_g.set_high();
					Timer::after_millis(500).await;
				}
			},
			Either::Second(_) => {
				clear_all();
				info!("Button pressed again");
				for _ in 0..3 {
					led_r.set_low();
					Timer::after_millis(500).await;
					led_r.set_high();
					Timer::after_millis(500).await;
				}
			},
		}

		Timer::after_millis(500).await;
	}
}
