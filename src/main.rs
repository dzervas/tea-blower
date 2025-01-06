#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{self, Input, Pull};
use embassy_rp::peripherals::{PIO0, USB};
use embassy_rp::pio::{Pio, InterruptHandler};
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_time::Timer;
use gpio::{Level, Output};
use log::*;

use defmt_rtt as _;
use panic_probe as _;
use smart_leds::colors::{BLUE, GREEN, RED, YELLOW};
use smart_leds::RGB8;

pub mod ws2812;
use ws2812::Ws2812;

const TIMER_SECS: u64 = 210;
const DEBOUNCE_MS: u64 = 200;
const DOUBLE_PRESS_MS: u64 = 500;

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
	let Pio { mut common, sm0, .. } = Pio::new(p.PIO0, Irqs);
	let mut led = Ws2812::new(&mut common, sm0, p.DMA_CH0, p.PIN_16);

	for _ in 0..3 {
		led.write(&GREEN).await;
		Timer::after_millis(250).await;
		led.write(&RGB8::default()).await;
		Timer::after_millis(250).await;
	}

	loop {
		button.wait_for_high().await;
		Timer::after_millis(DEBOUNCE_MS).await;

		info!("Button pressed");
		blower.set_high();

		let wait_time = match select(Timer::after_millis(DOUBLE_PRESS_MS), button.wait_for_high()).await {
			// Timed out
			Either::First(_) => {
				led.write(&BLUE).await;
				Timer::after_secs(2).await;
				led.write(&RGB8::default()).await;
				TIMER_SECS
			},
			// Double press
			Either::Second(_) => {
				info!("Double press");
				led.write(&YELLOW).await;
				Timer::after_secs(2).await;
				led.write(&RGB8::default()).await;
				TIMER_SECS * 2
			},
		};

		// Either the button is pressed again or the timer expires
		match select(Timer::after_secs(wait_time), button.wait_for_high()).await {
			Either::First(_) => {
				info!("Timer expired");
				for _ in 0..3 {
					led.write(&GREEN).await;
					Timer::after_millis(500).await;
					led.write(&RGB8::default()).await;
					Timer::after_millis(500).await;
				}
			},
			Either::Second(_) => {
				info!("Button pressed again");
				for _ in 0..3 {
					led.write(&RED).await;
					Timer::after_millis(500).await;
					led.write(&RGB8::default()).await;
					Timer::after_millis(500).await;
				}
			},
		}

		blower.set_low();
		Timer::after_millis(500).await;
	}
}
