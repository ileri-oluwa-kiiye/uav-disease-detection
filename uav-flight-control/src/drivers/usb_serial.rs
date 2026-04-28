//! Interrupt-driven USB CDC serial driver for WeAct Mini STM32H743
//! Uses USB2 (OTG_FS) on PA11/PA12

use stm32h7xx_hal::usb_hs::{UsbBus, USB2};
use stm32h7xx_hal::{pac, rcc};
use usb_device::prelude::*;
use usbd_serial::SerialPort;

static mut EP_MEM: [u32; 1024] = [0; 1024];
static mut USB_BUS: Option<usb_device::bus::UsbBusAllocator<UsbBus<USB2>>> = None;

pub type UsbDev = UsbDevice<'static, UsbBus<USB2>>;
pub type UsbSer = SerialPort<'static, UsbBus<USB2>>;

#[allow(static_mut_refs)] // USB bus is stored in a static mut ref, safe as long as init is called once
/// Initialize USB2 on PA11/PA12. Call once during setup.
pub fn init(
    usb_global: pac::OTG2_HS_GLOBAL,
    usb_device: pac::OTG2_HS_DEVICE,
    usb_pwrclk: pac::OTG2_HS_PWRCLK,
    pin_dm: stm32h7xx_hal::gpio::PA11<stm32h7xx_hal::gpio::Alternate<10>>,
    pin_dp: stm32h7xx_hal::gpio::PA12<stm32h7xx_hal::gpio::Alternate<10>>,
    prec: rcc::rec::Usb2Otg,
    clocks: &rcc::CoreClocks,
) -> (UsbDev, UsbSer) {
    let usb = USB2::new(
        usb_global, usb_device, usb_pwrclk, pin_dm, pin_dp, prec, clocks,
    );

    unsafe {
        USB_BUS = Some(UsbBus::new(usb, &mut EP_MEM));
        let bus_ref = USB_BUS.as_ref().unwrap();

        let serial = SerialPort::new(bus_ref);
        let dev = UsbDeviceBuilder::new(bus_ref, UsbVidPid(0x1209, 0x0001))
            .strings(&[StringDescriptors::default()
                .manufacturer("UAV")
                .product("Flight Controller")
                .serial_number("001")])
            .unwrap()
            .device_class(usbd_serial::USB_CLASS_CDC)
            .build();
        (dev, serial)
    }
}

/// Write bytes to USB serial. Non-blocking. Returns bytes written.
pub fn write(serial: &mut UsbSer, data: &[u8]) -> usize {
    match serial.write(data) {
        Ok(n) => n,
        Err(_) => 0,
    }
}

// Write all bytes, with bounded retries
pub fn write_all(serial: &mut UsbSer, data: &[u8]) {
    let mut offset = 0;
    let mut attempts = 0;
    while offset < data.len() && attempts < 50 {
        let n = write(serial, &data[offset..]);
        if n > 0 {
            offset += n;
            attempts = 0;
        } else {
            attempts += 1;
        }
    }
}
