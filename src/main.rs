#![no_std]
#![no_main]

use log::*;
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{self, Input, Pull};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_time::Timer;
use gpio::{Level, Output};
use defmt_rtt as _;
use panic_probe as _;

const TIMER_SECS: u64 = 210;
const DEBOUNCE_MS: u64 = 250;
const DOUBLE_PRESS_MS: u64 = 500;

bind_interrupts!(struct Irqs {
	USBCTRL_IRQ => InterruptHandler<USB>;
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

	loop {
		button.wait_for_high().await;
		Timer::after_millis(DEBOUNCE_MS).await;

		info!("Button pressed");
		blower.set_high();

		let wait_time = match select(Timer::after_secs(DOUBLE_PRESS_MS), button.wait_for_high()).await {
			// Timed out
			Either::First(_) => TIMER_SECS,
			// Double press
			Either::Second(_) => {
				info!("Double press");
				TIMER_SECS * 2
			},
		};

		Timer::after_millis(DEBOUNCE_MS).await;
		// Either the button is pressed again or the timer expires
		match select(Timer::after_secs(wait_time), button.wait_for_high()).await {
			Either::First(_) => info!("Timer expired"),
			Either::Second(_) => info!("Button pressed again"),
		}

		blower.set_low();
		Timer::after_millis(500).await;
	}
}
