use esp_hal::delay::Delay;

pub fn ms(ms: u32) {
    Delay::new().delay_millis(ms);
}

pub fn ns(ns: u32) {
    Delay::new().delay_nanos(ns);
}
