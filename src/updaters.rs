use std::time::{Duration, Instant};

use esp_idf_hal::delay::Delay;

use device::{Action, Device, Devices};

use crate::encoder::Encoder;

pub trait EncoderDevices {
    fn update_encoder_slider_behavior(&mut self, encoders: &mut Vec<Encoder>, delay_ms: u32);
}

impl EncoderDevices for Devices {
    fn update_encoder_slider_behavior(&mut self, encoders: &mut Vec<Encoder>, delay_ms: u32) {
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
                update_device(device, encoder, last_encoder_time, last_encoder_value, delay_ms.into());
            }
        }
        Delay::delay_ms(&delay, delay_ms);
    }
}

fn update_device(
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
                    device.updated = true;
                } else {
                    let _ = device.take_action(Action::Down(None));
                    device.updated = true;
                }
            }
            *last_encoder_time = Instant::now();
        }
        *last_encoder_value = encoder_value;
    }
}
