mod controller;
mod imu;

use std::cell::RefCell;
use std::ops::Deref;
use std::sync::Arc;
use bme280::i2c::BME280;
use embassy_time::Timer;
use embedded_hal_bus::i2c::MutexDevice;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio;
use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_hal::prelude::*;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::timer::{TimerConfig, TimerDriver};
use esp_idf_hal::uart::{AsyncUartDriver, UartConfig};
use mpu9250::{I2cDevice, ImuMeasurements, Mpu9250};
use nalgebra::Vector3;
use shared_bus::{BusManager, BusManagerStd, I2cProxy};
use tokio::sync::{Mutex, RwLock};
use crate::controller::Controller;

fn initialize_esp() {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    // eventfd is needed by our mio poll implementation.  Note you should set max_fds
    // higher if you have other code that may need eventfd.
    let config = esp_idf_sys::esp_vfs_eventfd_config_t {
        max_fds: 1,
        ..Default::default()
    };
    unsafe { esp_idf_sys::esp_vfs_eventfd_register(&config) };
}

fn main() -> anyhow::Result<()> {
    initialize_esp();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async_main())
}

async fn async_main() -> anyhow::Result<()> {
    log::info!("Tokio runtime loaded, starting main!");

    let peripherals = Peripherals::take()?;

    let mut timer = TimerDriver::new(peripherals.timer00, &TimerConfig::new())?;

    let i2c_config = I2cConfig::new().baudrate(100.kHz().into());
    let i2c = std::sync::Mutex::new(I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio8,
        peripherals.pins.gpio9,
        &i2c_config
    )?);

    let i2c_mutex: MutexDevice<I2cDriver>;

    unsafe {
        let test = std::ptr::from_ref(&i2c);
        i2c_mutex = embedded_hal_bus::i2c::MutexDevice::new(test.as_ref().unwrap());
        let i2c_bus: &'static _ = shared_bus::new_std!(embedded_hal_bus::i2c::MutexDevice<I2cDriver> = i2c_mutex).unwrap();
    }

    // i2c_bus.acquire_i2c()

    let uart_config = UartConfig::new().baudrate(416_666.Hz().into());
    let uart = AsyncUartDriver::new(
        peripherals.uart0,
        peripherals.pins.gpio21,
        peripherals.pins.gpio20,
        Option::<gpio::Gpio0>::None,
        Option::<gpio::Gpio1>::None,
        &uart_config,
    )?;

    let mut bmp = BME280::new(i2c_mutex, 0x76);
    bmp.init(&mut FreeRtos).unwrap();

    loop {
        let m = bmp.measure(&mut FreeRtos).unwrap();
        log::info!("{:?}", m);
    }

    // let imu = Arc::new(Imu::new(&i2c_bus));
    let controller = Arc::new(Controller::new(uart));

    tokio::try_join!(
        // main_loop(controller.clone(), imu.clone()),
        // imu.read_loop(),
        controller.read_loop(),
    )?;

    Ok(())
}

#[derive(Debug)]
pub struct Axes {
    pub accel: Vector3<f32>,
    pub gyro: Vector3<f32>,
    pub temp: f32,
    // TODO this fucks
    pub elev: f32,
}

// pub struct Imu {
//     imu: Mutex<Mpu9250<I2cDevice<I2cProxy<'static, std::sync::Mutex<I2cDriver<'static>>>>, mpu9250::Imu>>,
//     axes: RwLock<Axes>,
// }
//
// impl Imu {
//     pub fn new(bus: &'static BusManager<std::sync::Mutex<I2cDriver>>) -> Self {
//         let mut bmp = BME280::new(bus.acquire_i2c(), 0x76);
//
//         let mut imu = Mpu9250::imu_default(bus.acquire_i2c(), &mut FreeRtos).unwrap();
//         log::debug!("Connected to IMU {:?}", imu.who_am_i());
//
//         Self {
//             imu: Mutex::new(imu),
//             axes: RwLock::new(Axes {
//                 accel: Default::default(),
//                 gyro: Default::default(),
//                 temp: 0.0,
//                 elev: 0.0,
//             })
//         }
//     }
//     pub async fn read_loop(&self) -> anyhow::Result<()> {
//         loop {
//             let all: ImuMeasurements<Vector3<f32>> = self.imu.lock().await.all().unwrap();
//             {
//                 let mut axes = self.axes.write().await;
//                 axes.accel = all.accel;
//                 axes.gyro = all.gyro;
//                 axes.temp = all.temp;
//             }
//             Timer::after_millis(10).await;
//         }
//     }
// }
//
// async fn main_loop(controller: Arc<Controller>, imu: Arc<Imu>) -> anyhow::Result<()> {
//     loop {
//         log::info!("{:?}", *controller.channels.read().await);
//         log::info!("{:?}", *imu.axes.read().await);
//         Timer::after_millis(100).await;
//     }
// }
