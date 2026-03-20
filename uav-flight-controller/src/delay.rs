use esp_hal::delay::Delay;

#[inline(always)]
pub fn ms(ms: u32) {
    Delay::new().delay_millis(ms);
}

#[inline(always)]
pub fn ns(ns: u32) {
    Delay::new().delay_nanos(ns);
}
