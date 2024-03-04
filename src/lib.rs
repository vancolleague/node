use std::{
    collections::HashMap,
    default::Default,
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};

use heapless;
use uuid::Uuid;

pub use embedded_svc::http::Method;
use embedded_svc::{http::server::Request, io::Write};
pub use esp_idf_hal::gpio::{AnyInputPin, InputPin, PinDriver};
pub use esp_idf_hal::ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver};
pub use esp_idf_hal::pcnt::Pcnt;
pub use esp_idf_hal::units::FromValueType;
use esp_idf_hal::{delay::Delay, modem::Modem, peripherals::Peripherals, units::Hertz};
use esp_idf_svc::http::server::Configuration as SVC_Configuration;
pub use esp_idf_svc::io::EspIOError;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    http::server::{EspHttpConnection, EspHttpServer},
    nvs::EspDefaultNvsPartition,
    wifi::{ClientConfiguration, Configuration, EspWifi},
};
//pub use esp_idf_hal::ledc::{config::LedcDriver, LedcTimerDriver, TimerConfig};

pub use device;
use device::{Action, Device, Devices};

pub mod encoder;
pub mod updaters;
use updaters::EncoderDevices;
//pub mod wrappers;
//pub use encoder::{update_slider_type_device_from_encoder, Encoder, EncoderPeripheralData};

pub struct Node {
    pub ssid: String,
    pub password: String,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            ssid: String::default(),
            password: String::default(),
        }
    }
}

impl Node {
    pub fn setup() -> Peripherals {
        esp_idf_svc::sys::link_patches();

        esp_idf_svc::log::EspLogger::initialize_default();

        log::info!("hello log");

        Peripherals::take().expect("Something went wrong taking the Peripherals.")
    }

    //#[cfg(all(not(feature = "riscv-ulp-hal"), any(esp32, esp32s2, esp32s3)))]
    pub fn run(&mut self, devices: Devices, modem: Modem) -> Result<(), EspIOError> {
        let sys_loop = EspSystemEventLoop::take()?;
        let nvs = EspDefaultNvsPartition::take()?;
        let mut wifi_driver = EspWifi::new(modem, sys_loop, Some(nvs)).unwrap();
        wifi_driver.set_configuration(&Configuration::Client(ClientConfiguration {
            ssid: heapless::String::try_from(self.ssid.as_str()).unwrap(),
            password: heapless::String::try_from(self.password.as_str()).unwrap(),
            ..Default::default()
        }))?;
        wifi_driver.start()?;
        wifi_driver.connect()?;
        while !wifi_driver.is_connected().unwrap() {
            let config = wifi_driver.get_configuration().unwrap();
            println!("Waiting for station {:?}", config);
        }
        println!("Should be connected now");

        let mut server = EspHttpServer::new(&SVC_Configuration::default()).unwrap();
        /*for (path, method, handler) in handlers.iter() {
            server.fn_handler(path.as_str(), method, handler)
            .unwrap();
        }*/
        /*server
        .fn_handler("/", Method::Get, |request| {
            let mut response = request.into_ok_response()?;
            response.write_all("payload!!!!!".as_bytes())?;
            Ok(())
        })
        .unwrap();*/
        let devices_clone = devices.clone();
        server
            .fn_handler("/status", Method::Get, move |request| {
                if &request.uri().len() < &8_usize {
                    let _ = exit_early(request, "Bad Status command given", 422);
                    return Ok(());
                    //return Ok::<(), EspIOError>(());
                }
                let query = &request.uri()[8..].to_string().to_lowercase();
                let query: HashMap<_, _> = querystring::querify(query).into_iter().collect();
                if query.get("device").is_some() {
                    let d = query.get("device").unwrap();
                    let d = d.replace("%20", " ");
                    for device in devices_clone.devices.lock().unwrap().iter() {
                        if device.name == d {
                            let mut response = request.into_ok_response()?;
                            let _ = response.write_all(&device.to_json().into_bytes()[..]);
                            //return Ok(());
                            return Ok::<(), EspIOError>(());
                        }
                    }
                    let _ = exit_early(request, "Device name not found", 422);
                    return Ok(());
                } else if query.get("uuid").is_some() {
                    let u = query.get("uuid").unwrap();
                    for device in devices_clone.devices.lock().unwrap().iter() {
                        if &device.uuid.to_string().as_str() == u {
                            let mut response = request.into_ok_response()?;
                            let _ = response.write_all(&device.to_json().into_bytes()[..]);
                            //return Ok(());
                            return Ok::<(), EspIOError>(());
                        }
                    }
                    let _ = exit_early(request, "Device name not found", 422);
                    //return Ok(());
                    return Ok::<(), EspIOError>(());
                } else {
                    let _ = exit_early(request, "No Device name given", 422);
                    //return Ok(());
                    return Ok::<(), EspIOError>(());
                }
            })
            .unwrap();
        let devices_clone = devices.clone();
        server
            .fn_handler("/devices", Method::Get, move |request| {
                let mut devices = HashMap::new();
                {
                    for device in devices_clone.devices.lock().unwrap().iter() {
                        devices.insert(device.name.clone(), device.clone());
                    }
                }
                let payload = serde_json::json!(devices);
                let mut response = request.into_ok_response()?;
                response.write_all(payload.to_string().as_bytes())?;
                //Ok(())
                return Ok::<(), EspIOError>(());
            })
            .unwrap();
        let devices_clone = devices.clone();
        server
            .fn_handler("/command", Method::Get, move |request| {
                if &request.uri().len() < &9_usize {
                    let _ = exit_early(request, "Bad Command given", 422);
                    //return Ok(());
                    return Ok::<(), EspIOError>(());
                }
                let query = &request.uri()[9..].to_string();
                let query: HashMap<_, _> = querystring::querify(query).into_iter().collect();
                let target: Option<usize> = match query.get("target") {
                    Some(t_text) => match t_text.parse::<usize>() {
                        Ok(t_num) => {
                            if t_num > 7 {
                                let _ = exit_early(request, "Target must be >= 0 & < 8", 422);
                                //return Ok(());
                                return Ok::<(), EspIOError>(());
                            }
                            Some(t_num)
                        }
                        Err(_) => {
                            if t_text != &"" {
                                let _ = exit_early(request, "Bad Target given", 422);
                                //return Ok(());
                                return Ok::<(), EspIOError>(());
                            }
                            None
                        }
                    },
                    None => None,
                };
                let action = match query.get("action") {
                    Some(a) => match Action::from_str(&a.to_lowercase(), target) {
                        Ok(a) => a,
                        Err(_) => {
                            let _ = exit_early(request, "Bad Action name given", 422);
                            //return Ok(());
                            return Ok::<(), EspIOError>(());
                        }
                    },
                    None => {
                        let _ = exit_early(request, "No Action given", 422);
                        //return Ok(());
                        return Ok::<(), EspIOError>(());
                    }
                };
                match query.get("uuid") {
                    Some(u) => match Uuid::parse_str(u) {
                        Ok(uuid) => {
                            for device in devices_clone.devices.lock().unwrap().iter_mut() {
                                if uuid == device.uuid {
                                    let _ = device.take_action(action);
                                    let mut response = request.into_ok_response()?;
                                    let _ = response.write_all(&device.to_json().into_bytes());
                                    //return Ok(());
                                    return Ok::<(), EspIOError>(());
                                }
                            }
                            let _ = exit_early(request, "Uuid not found among devices", 422);
                            //`return Ok(());
                            return Ok::<(), EspIOError>(());
                        }
                        Err(_) => {
                            let _ = exit_early(request, "Bad Uuid given", 422);
                            //return Ok(());
                            return Ok::<(), EspIOError>(());
                        }
                    },
                    None => {
                        let _ = exit_early(request, "Uuid field not given", 422);
                        //return Ok(());
                        return Ok::<(), EspIOError>(());
                    }
                };
            })
            .unwrap();

        loop {
            println!(
                "IP info: {:?}",
                wifi_driver.sta_netif().get_ip_info().unwrap()
            );
            sleep(Duration::new(100, 0));
        }
        Ok(())
    }
}

pub fn get_frequencies(devices: &Devices) -> Vec<Hertz> {
    devices
        .devices
        .lock()
        .unwrap()
        .iter()
        .map(|d| d.freq_Hz.Hz())
        .collect()
}

pub fn get_max_duty_cycles(drivers: &Vec<LedcDriver>) -> Vec<u32> {
    let mut max_duty_cycles = Vec::with_capacity(drivers.len());
    for driver in drivers {
        max_duty_cycles.push(driver.get_max_duty());
    }
    max_duty_cycles
}

fn exit_early<'a>(
    request: Request<&mut EspHttpConnection<'a>>,
    message: &str,
    code: u16,
) -> Result<(), EspIOError> {
    let mut response = request.into_status_response(code)?;
    let _ = response.write_all(message.as_bytes());
    Ok(())
}

pub trait DevicesDutyCycles {
    fn update_duty_cycles(
        &mut self,
        drivers: Vec<LedcDriver>,
        max_duty_cycles: Vec<u32>,
        delay_ms: u32,
    );
}

impl DevicesDutyCycles for Devices {
    fn update_duty_cycles(
        &mut self,
        mut drivers: Vec<LedcDriver>,
        max_duty_cycles: Vec<u32>,
        delay_ms: u32,
    ) {
        let delay = Delay::new(100);
        loop {
            {
                for ((device, driver), max_duty) in self
                    .devices
                    .lock()
                    .unwrap()
                    .iter_mut()
                    .zip(drivers.iter_mut())
                    .zip(max_duty_cycles.iter())
                {
                    if device.needs_hardware_duty_cycle_update() {
                        println!("Updating: {:?}", device);
                        let duty_cycle = device.get_and_update_duty_cycle(max_duty);
                        let _ = driver.set_duty(duty_cycle);
                    }
                }
            }
            delay.delay_ms(delay_ms);
        }
    }
}
/*
pub fn update_device_duty_cycles(
    devices: Devices,
    mut drivers: Vec<LedcDriver>,
    max_duty_cycles: Vec<u32>,
    delay_ms: u32,
) {
    let delay = Delay::new(100);
    loop {
        {
            for ((device, driver), max_duty) in devices
                .devices
                .lock()
                .unwrap()
                .iter_mut()
                .zip(drivers.iter_mut())
                .zip(max_duty_cycles.iter())
            {
                if device.updated {
                    println!("Updating: {:?}", device);
                    let duty_cycle = device.get_and_update_duty_cycle(max_duty);
                    let _ = driver.set_duty(duty_cycle);
                }
            }
        }
        delay.delay_ms(delay_ms);
    }
}*/
