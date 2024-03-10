use std::time::{Duration, Instant};

use esp_idf_hal::{
    delay::Delay,
    gpio::{AnyInputPin, Input, InputPin, Pin, PinDriver},
    peripheral::Peripheral,
    peripherals::Peripherals,
};

use device::{Action, Behavior, Device, Devices};

use crate::encoder::Encoder;

pub trait EncoderDevices {
    fn take_actions_slider_encoder(&mut self, encoders: &mut Vec<Encoder>, delay_ms: u32);
    fn take_actions_reversible_slider_encoder(
        &mut self,
        encoders: Vec<Encoder>,
        reverse_pins: Vec<PinDriver<'static, AnyInputPin, Input>>,
        delay_ms: u32,
    );
}

impl EncoderDevices for Devices {
    fn take_actions_slider_encoder(&mut self, encoders: &mut Vec<Encoder>, delay_ms: u32) {
        let length = { self.devices.lock().unwrap().len() };
        let mut last_encoder_values = vec![0; length];
        let mut last_encoder_times = vec![Instant::now(); length];
        let delay = Delay::new(delay_ms);
        loop {
            let mut devices_guard = self.devices.lock().unwrap();
            for (((device, encoder), last_encoder_time), last_encoder_value) in devices_guard
                .iter_mut()
                .zip(encoders.iter_mut())
                .zip(last_encoder_times.iter_mut())
                .zip(last_encoder_values.iter_mut())
            {
                if device.behavior == Behavior::Slider {
                    update_device_from_encoder(
                        device,
                        encoder,
                        last_encoder_time,
                        last_encoder_value,
                        delay_ms.into(),
                    );
                }
            }
            delay.delay_ms(delay_ms);
        }
    }

    fn take_actions_reversible_slider_encoder(
        &mut self,
        mut encoders: Vec<Encoder>,
        mut reverse_pins: Vec<PinDriver<'static, AnyInputPin, Input>>,
        delay_ms: u32,
    ) {
        let length = { self.devices.lock().unwrap().len() };
        let mut last_encoder_values = vec![0; length];
        let mut last_encoder_times = vec![Instant::now(); length];
        let mut last_click_times: Vec<Option<Instant>> = vec![None; length];
        let delay = Delay::new(delay_ms);
        loop {
            let mut devices_guard = self.devices.lock().unwrap();
            for (
                ((((device, encoder), reverse_pin), last_encoder_time), last_encoder_value),
                mut last_click_time,
            ) in devices_guard
                .iter_mut()
                .zip(encoders.iter_mut())
                .zip(reverse_pins.iter_mut())
                .zip(last_encoder_times.iter_mut())
                .zip(last_encoder_values.iter_mut())
                .zip(last_click_times.iter_mut())
            {
                if device.behavior == Behavior::ReversableSlider {
                    update_reversable_device_from_pin_click(
                        device,
                        last_click_time,
                        reverse_pin,
                        delay_ms,
                    );
                    update_device_from_encoder(
                        device,
                        encoder,
                        last_encoder_time,
                        last_encoder_value,
                        delay_ms.into(),
                    );
                }
            }
        }

        Delay::delay_ms(&delay, delay_ms);
    }
}

/// Update a device's status
///
/// Updates the software device's status status based in inputs (hardware
/// inputs or )
/// Doesn't update the hardware's stuats or define what should be done
/// in order to do so
fn update_reversable_device_from_pin_click(
    device: &mut Device,
    last_click_time: &mut Option<Instant>,
    reverse_pin: &mut PinDriver<'static, AnyInputPin, Input>,
    delay_ms: u32,
) {
    if reverse_pin.is_high() {
        if last_click_time.is_none() {
            *last_click_time = Some(Instant::now());
        } else {
            let current_time = Instant::now();
            if current_time.duration_since(last_click_time.unwrap())
                > Duration::from_millis(delay_ms.into())
            {
                let _ = device.take_action(Action::Reverse);
            }
        }
    } else if last_click_time.is_some() {
        *last_click_time = None;
    }
}

fn update_device_from_encoder(
    device: &mut Device,
    encoder: &mut Encoder,
    last_encoder_time: &mut Instant,
    last_encoder_value: &mut i32,
    delay_ms: u64,
) {
    let encoder_value = encoder.get_value().unwrap();
    if encoder_value != *last_encoder_value {
        let current_time = Instant::now();
        let time_since_last_check = current_time.duration_since(*last_encoder_time);
        if time_since_last_check > Duration::from_millis(delay_ms) {
            {
                if encoder_value > *last_encoder_value {
                    let _ = device.take_action(Action::Up(None));
                } else {
                    let _ = device.take_action(Action::Down(None));
                }
            }
            *last_encoder_time = Instant::now();
        }
        *last_encoder_value = encoder_value;
    }
}
