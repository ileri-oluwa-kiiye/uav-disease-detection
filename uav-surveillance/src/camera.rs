use std::ptr::NonNull;

use esp_idf_svc::sys::camera::*;

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[error("Failed to initialize camera: {0}")]
pub struct CameraInitError(esp_err_t);

// Pin assignments
const CAM_PIN_D0: i32 = 11;
const CAM_PIN_D1: i32 = 9;
const CAM_PIN_D2: i32 = 8;
const CAM_PIN_D3: i32 = 10;
const CAM_PIN_D4: i32 = 12;
const CAM_PIN_D5: i32 = 18;
const CAM_PIN_D6: i32 = 17;
const CAM_PIN_D7: i32 = 16;

const CAM_PIN_XCLK: i32 = 15;
const CAM_PIN_PCLK: i32 = 13;
const CAM_PIN_VSYNC: i32 = 6;

const CAM_PIN_HREF: i32 = 7;
const CAM_PIN_SDA: i32 = 4;
const CAM_PIN_SCL: i32 = 5;
const CAM_PIN_PWDN: i32 = -1;
const CAM_PIN_RESET: i32 = -1;

pub fn init() -> Result<(), CameraInitError> {
    let config = camera_config_t {
        pin_pwdn: CAM_PIN_PWDN,
        pin_reset: CAM_PIN_RESET,
        pin_xclk: CAM_PIN_XCLK,
        __bindgen_anon_1: camera_config_t__bindgen_ty_1 {
            pin_sccb_sda: CAM_PIN_SDA,
        },
        __bindgen_anon_2: camera_config_t__bindgen_ty_2 {
            pin_sccb_scl: CAM_PIN_SCL,
        },
        pin_d7: CAM_PIN_D7,
        pin_d6: CAM_PIN_D6,
        pin_d5: CAM_PIN_D5,
        pin_d4: CAM_PIN_D4,
        pin_d3: CAM_PIN_D3,
        pin_d2: CAM_PIN_D2,
        pin_d1: CAM_PIN_D1,
        pin_d0: CAM_PIN_D0,
        pin_vsync: CAM_PIN_VSYNC,
        pin_href: CAM_PIN_HREF,
        pin_pclk: CAM_PIN_PCLK,
        xclk_freq_hz: 20_000_000,
        ledc_timer: ledc_timer_t_LEDC_TIMER_0,
        ledc_channel: ledc_channel_t_LEDC_CHANNEL_0,
        pixel_format: pixformat_t_PIXFORMAT_JPEG,
        frame_size: framesize_t_FRAMESIZE_VGA,
        jpeg_quality: 12,
        fb_count: 2,
        fb_location: camera_fb_location_t_CAMERA_FB_IN_PSRAM,
        grab_mode: camera_grab_mode_t_CAMERA_GRAB_LATEST,
        sccb_i2c_port: -1,
    };

    let ret = unsafe { esp_camera_init(&config) };
    if ret != ESP_OK {
        return Err(CameraInitError(ret));
    }
    Ok(())
}

pub struct Frame {
    fb: *mut camera_fb_t,
}

impl Frame {
    pub fn data(&self) -> &[u8] {
        unsafe {
            let fb = &*self.fb;
            std::slice::from_raw_parts(fb.buf, fb.len)
        }
    }

    pub fn width(&self) -> usize {
        unsafe { (&*self.fb).width }
    }

    pub fn height(&self) -> usize {
        unsafe { (&*self.fb).height }
    }

    pub fn len(&self) -> usize {
        unsafe { (&*self.fb).len }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        unsafe {
            esp_camera_fb_return(self.fb);
        }
    }
}

pub fn capture() -> Option<Frame> {
    let fb = unsafe { esp_camera_fb_get() };
    (!fb.is_null()).then_some(Frame { fb })
}
