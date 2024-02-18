use std::cmp::min;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use esp_idf_hal::delay::Delay;
use esp_idf_hal::gpio::AnyInputPin;
pub use esp_idf_hal::gpio::InputPin;
use esp_idf_hal::pcnt::*;
pub use esp_idf_hal::peripheral::Peripheral;
pub use esp_idf_svc::hal::ledc::LedcDriver;
use esp_idf_sys::EspError;

use device::{Action, Device};

const LOW_LIMIT: i16 = -100;
const HIGH_LIMIT: i16 = 100;

pub struct Encoder<'d> {
    unit: PcntDriver<'d>,
    approx_value: Arc<AtomicI32>,
}

impl<'d> Encoder<'d> {
    pub fn new<PCNT: Pcnt>(
        pcnt: impl Peripheral<P = PCNT> + 'd,
        pin_a: impl Peripheral<P = impl InputPin> + 'd,
        pin_b: impl Peripheral<P = impl InputPin> + 'd,
    ) -> Result<Self, EspError> {
        let mut unit = PcntDriver::new(
            pcnt,
            Some(pin_a),
            Some(pin_b),
            Option::<AnyInputPin>::None,
            Option::<AnyInputPin>::None,
        )?;
        unit.channel_config(
            PcntChannel::Channel0,
            PinIndex::Pin0,
            PinIndex::Pin1,
            &PcntChannelConfig {
                lctrl_mode: PcntControlMode::Reverse,
                hctrl_mode: PcntControlMode::Keep,
                pos_mode: PcntCountMode::Decrement,
                neg_mode: PcntCountMode::Increment,
                counter_h_lim: HIGH_LIMIT,
                counter_l_lim: LOW_LIMIT,
            },
        )?;
        unit.channel_config(
            PcntChannel::Channel1,
            PinIndex::Pin1,
            PinIndex::Pin0,
            &PcntChannelConfig {
                lctrl_mode: PcntControlMode::Reverse,
                hctrl_mode: PcntControlMode::Keep,
                pos_mode: PcntCountMode::Increment,
                neg_mode: PcntCountMode::Decrement,
                counter_h_lim: HIGH_LIMIT,
                counter_l_lim: LOW_LIMIT,
            },
        )?;

        unit.set_filter_value(min(10 * 80, 1023))?;
        //       unit.set_filter_value(min(1023/2, 1023))?;
        unit.filter_enable()?;

        let approx_value = Arc::new(AtomicI32::new(0));
        // unsafe interrupt code to catch the upper and lower limits from the encoder
        // and track the overflow in `value: Arc<AtomicI32>` - I plan to use this for
        // a wheeled robot's odomerty
        unsafe {
            let approx_value = approx_value.clone();
            unit.subscribe(move |status| {
                let status = PcntEventType::from_repr_truncated(status);
                if status.contains(PcntEvent::HighLimit) {
                    approx_value.fetch_add(HIGH_LIMIT as i32, Ordering::SeqCst);
                }
                if status.contains(PcntEvent::LowLimit) {
                    approx_value.fetch_add(LOW_LIMIT as i32, Ordering::SeqCst);
                }
            })?;
        }
        unit.event_enable(PcntEvent::HighLimit)?;
        unit.event_enable(PcntEvent::LowLimit)?;
        unit.counter_pause()?;
        unit.counter_clear()?;
        unit.counter_resume()?;

        Ok(Self { unit, approx_value })
    }

    pub fn get_value(&self) -> Result<i32, EspError> {
        let value =
            self.approx_value.load(Ordering::Relaxed) + self.unit.get_counter_value()? as i32;
        Ok(value)
    }
}

pub fn update_slider_type_devices(
    mut encoders: Vec<Encoder>,
    devices: Arc<Mutex<Vec<Device>>>,
    delay_ms: u32,
) {
    let mut last_encoder_values = Vec::from([0i32, 0i32]);
    let mut last_encoder_times = Vec::from([Instant::now(), Instant::now()]);
    let delay = Delay::new(delay_ms);
    loop {
        {
            let mut devices_guard = devices.lock().unwrap();
            for (((device, encoder), last_encoder_time), last_encoder_value) in devices_guard
                .iter_mut()
                .zip(encoders.iter_mut())
                .zip(last_encoder_times.iter_mut())
                .zip(last_encoder_values.iter_mut())
            {
                update_device(device, encoder, last_encoder_time, last_encoder_value);
            }
        }
        //Delay::delay_ms(100u32);
        Delay::delay_ms(&delay, delay_ms);
    }
}

fn update_device(
    device: &mut Device,
    encoder: &mut Encoder,
    last_encoder_time: &mut Instant,
    last_encoder_value: &mut i32,
) {
    let encoder_value = encoder.get_value().unwrap();
    if encoder_value != *last_encoder_value {
        let current_time = Instant::now();
        let time_since_last_check = current_time.duration_since(*last_encoder_time);
        if time_since_last_check > Duration::from_millis(100) {
            {
                if encoder_value > *last_encoder_value {
                    let _ = device.take_action(Action::Up { amount: None });
                    device.updated = true;
                } else {
                    let _ = device.take_action(Action::Down { amount: None });
                    device.updated = true;
                }
            }
            *last_encoder_time = Instant::now();
        }
        *last_encoder_value = encoder_value;
    }
}
