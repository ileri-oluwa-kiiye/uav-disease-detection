//! ICM-42688-P 6-axis IMU driver over SPI
//!
//! Uses embedded-hal 0.2 traits (compatible with stm32h7xx-hal v0.16)
//! Register map reference: DS-000347 Rev 1.6

use embedded_hal::blocking::spi::Transfer;
use embedded_hal::blocking::spi::Write;
use embedded_hal::digital::v2::OutputPin;
use micromath::F32Ext;
// ---- WHO_AM_I ----
const WHO_AM_I: u8 = 0x75;
const WHO_AM_I_VALUE: u8 = 0x47;

// ---- Bank 0 registers ----
const REG_DEVICE_CONFIG: u8 = 0x11;
const REG_TEMP_DATA1: u8 = 0x1D;
const REG_INT_STATUS: u8 = 0x2D;
const REG_PWR_MGMT0: u8 = 0x4E;
const REG_GYRO_CONFIG0: u8 = 0x4F;
const REG_ACCEL_CONFIG0: u8 = 0x50;
const REG_GYRO_ACCEL_CONFIG0: u8 = 0x52;
const REG_BANK_SEL: u8 = 0x76;

// SPI read bit
const SPI_READ: u8 = 0x80;

/// Gyroscope full-scale range
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum GyroRange {
    Dps2000 = 0b000 << 5,
    Dps1000 = 0b001 << 5,
    Dps500 = 0b010 << 5,
    Dps250 = 0b011 << 5,
    Dps125 = 0b100 << 5,
}

impl GyroRange {
    pub const fn sensitivity(self) -> f32 {
        match self {
            GyroRange::Dps2000 => 16.4,
            GyroRange::Dps1000 => 32.8,
            GyroRange::Dps500 => 65.5,
            GyroRange::Dps250 => 131.0,
            GyroRange::Dps125 => 262.0,
        }
    }
}

/// Accelerometer full-scale range
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum AccelRange {
    G16 = 0b000 << 5,
    G8 = 0b001 << 5,
    G4 = 0b010 << 5,
    G2 = 0b011 << 5,
}

impl AccelRange {
    pub fn sensitivity(self) -> f32 {
        match self {
            AccelRange::G16 => 2048.0,
            AccelRange::G8 => 4096.0,
            AccelRange::G4 => 8192.0,
            AccelRange::G2 => 16384.0,
        }
    }
}

/// Output data rate
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum Odr {
    Hz32000 = 0x01,
    Hz16000 = 0x02,
    Hz8000 = 0x03,
    Hz4000 = 0x04,
    Hz2000 = 0x05,
    Hz1000 = 0x06,
    Hz200 = 0x07,
    Hz100 = 0x08,
    Hz50 = 0x09,
}

/// Raw 6-axis reading
#[derive(Debug, Default, Clone, Copy)]
pub struct RawReading {
    pub accel_x: i16,
    pub accel_y: i16,
    pub accel_z: i16,
    pub gyro_x: i16,
    pub gyro_y: i16,
    pub gyro_z: i16,
    pub temp_raw: i16,
}

/// Scaled reading in physical units
#[derive(Debug, Default, Clone, Copy)]
pub struct ScaledReading {
    pub accel_x: f32,
    pub accel_y: f32,
    pub accel_z: f32,
    pub gyro_x: f32,
    pub gyro_y: f32,
    pub gyro_z: f32,
    pub temp_c: f32,
}

/// ICM-42688-P driver error
#[derive(Debug)]
pub enum Error<SpiE, PinE> {
    Spi(SpiE),
    Pin(PinE),
    WhoAmI(u8),
    MotionDetected(f32),
}

/// ICM-42688-P driver
pub struct Icm42688p<SPI, CS> {
    spi: SPI,
    cs: CS,
    gyro_range: GyroRange,
    accel_range: AccelRange,
    gyro_bias: [f32; 3],
}

impl<SPI, CS, SpiE, PinE> Icm42688p<SPI, CS>
where
    SPI: Transfer<u8, Error = SpiE> + Write<u8, Error = SpiE>,
    CS: OutputPin<Error = PinE>,
{
    /// Create a new driver instance
    pub fn new(spi: SPI, cs: CS) -> Self {
        Self {
            spi,
            cs,
            gyro_range: GyroRange::Dps2000,
            accel_range: AccelRange::G16,
            gyro_bias: [0.0; 3],
        }
    }

    /// Initialize the ICM-42688-P
    pub fn init(&mut self) -> Result<(), Error<SpiE, PinE>> {
        self.cs.set_high().map_err(Error::Pin)?;
        cortex_m::asm::delay(10_000);

        // Soft reset
        self.write_reg(REG_DEVICE_CONFIG, 0x01)?;
        cortex_m::asm::delay(1_000_000);

        // Verify WHO_AM_I
        let who = self.read_reg(WHO_AM_I)?;
        if who != WHO_AM_I_VALUE {
            return Err(Error::WhoAmI(who));
        }

        // Select bank 0
        self.write_reg(REG_BANK_SEL, 0x00)?;

        // Configure gyro: 2000 dps, 1kHz ODR
        self.set_gyro_config(GyroRange::Dps2000, Odr::Hz1000)?;

        // Configure accel: 16g, 1kHz ODR
        self.set_accel_config(AccelRange::G16, Odr::Hz1000)?;

        // Filter bandwidth ODR/4
        self.write_reg(REG_GYRO_ACCEL_CONFIG0, 0x44)?;

        // Power on gyro + accel in low-noise mode
        self.write_reg(REG_PWR_MGMT0, 0x0F)?;

        // Wait for stabilization
        cortex_m::asm::delay(1_000_000);

        Ok(())
    }

    pub fn set_gyro_config(&mut self, range: GyroRange, odr: Odr) -> Result<(), Error<SpiE, PinE>> {
        self.gyro_range = range;
        self.write_reg(REG_GYRO_CONFIG0, range as u8 | odr as u8)
    }

    pub fn set_accel_config(
        &mut self,
        range: AccelRange,
        odr: Odr,
    ) -> Result<(), Error<SpiE, PinE>> {
        self.accel_range = range;
        self.write_reg(REG_ACCEL_CONFIG0, range as u8 | odr as u8)
    }

    /// Read raw accelerometer, gyroscope, and temperature data
    pub fn read_raw(&mut self) -> Result<RawReading, Error<SpiE, PinE>> {
        let mut buf = [0u8; 15];
        buf[0] = REG_TEMP_DATA1 | SPI_READ;

        self.cs.set_low().map_err(Error::Pin)?;
        self.spi.transfer(&mut buf).map_err(Error::Spi)?;
        self.cs.set_high().map_err(Error::Pin)?;

        Ok(RawReading {
            temp_raw: i16::from_be_bytes([buf[1], buf[2]]),
            accel_x: i16::from_be_bytes([buf[3], buf[4]]),
            accel_y: i16::from_be_bytes([buf[5], buf[6]]),
            accel_z: i16::from_be_bytes([buf[7], buf[8]]),
            gyro_x: i16::from_be_bytes([buf[9], buf[10]]),
            gyro_y: i16::from_be_bytes([buf[11], buf[12]]),
            gyro_z: i16::from_be_bytes([buf[13], buf[14]]),
        })
    }

    /// Read scaled data in physical units
    pub fn read_scaled(&mut self) -> Result<ScaledReading, Error<SpiE, PinE>> {
        let raw = self.read_raw()?;
        let accel_sens = self.accel_range.sensitivity();
        let gyro_sens = self.gyro_range.sensitivity();

        Ok(ScaledReading {
            accel_x: raw.accel_x as f32 / accel_sens,
            accel_y: raw.accel_y as f32 / accel_sens,
            accel_z: raw.accel_z as f32 / accel_sens,
            gyro_x: raw.gyro_x as f32 / gyro_sens - self.gyro_bias[0],
            gyro_y: raw.gyro_y as f32 / gyro_sens - self.gyro_bias[1],
            gyro_z: raw.gyro_z as f32 / gyro_sens - self.gyro_bias[2],
            temp_c: (raw.temp_raw as f32 / 132.48) + 25.0,
        })
    }

    /// Check if data is ready
    pub fn data_ready(&mut self) -> Result<bool, Error<SpiE, PinE>> {
        let status = self.read_reg(REG_INT_STATUS)?;
        Ok(status & 0x08 != 0)
    }

    /// Read WHO_AM_I register
    pub fn who_am_i(&mut self) -> Result<u8, Error<SpiE, PinE>> {
        self.read_reg(WHO_AM_I)
    }

    fn read_reg(&mut self, reg: u8) -> Result<u8, Error<SpiE, PinE>> {
        let mut buf = [reg | SPI_READ, 0x00];
        self.cs.set_low().map_err(Error::Pin)?;
        self.spi.transfer(&mut buf).map_err(Error::Spi)?;
        self.cs.set_high().map_err(Error::Pin)?;
        Ok(buf[1])
    }

    fn write_reg(&mut self, reg: u8, val: u8) -> Result<(), Error<SpiE, PinE>> {
        self.cs.set_low().map_err(Error::Pin)?;
        self.spi.write(&[reg, val]).map_err(Error::Spi)?;
        self.cs.set_high().map_err(Error::Pin)?;
        Ok(())
    }

    /// Average `samples` stationary gyro readings and store the result as a
    /// zero-rate bias. MUST be called with the craft completely still.
    /// Returns the measured bias, or NotStationary if the craft moved.
    pub fn calibrate_gyro(&mut self, samples: u16) -> Result<[f32; 3], Error<SpiE, PinE>> {
        const MAX_STD_DPS: f32 = 2.0; // per-axis std-dev gate; loosen if your bench trips it

        self.gyro_bias = [0.0; 3]; // measure raw rate

        // Discard ~250 ms of post-init settling before measuring — the sensor
        // is noisier right after reset/power-on and can throw transients.
        for _ in 0..250 {
            let mut tries = 0u32;
            while !self.data_ready()? {
                tries += 1;
                if tries > 100_000 {
                    break;
                }
            }
            let _ = self.read_scaled()?;
        }

        // Accumulate sum and sum-of-squares for mean + std-dev.
        let mut sum = [0.0f32; 3];
        let mut sum_sq = [0.0f32; 3];
        for _ in 0..samples {
            let mut tries = 0u32;
            while !self.data_ready()? {
                tries += 1;
                if tries > 100_000 {
                    break;
                }
            }
            let r = self.read_scaled()?;
            let g = [r.gyro_x, r.gyro_y, r.gyro_z];
            for i in 0..3 {
                sum[i] += g[i];
                sum_sq[i] += g[i] * g[i];
            }
        }

        let n = samples as f32;
        let mean = [sum[0] / n, sum[1] / n, sum[2] / n];

        // Gate on std-dev (robust to single transients, unlike peak-to-peak).
        for i in 0..3 {
            let var = (sum_sq[i] / n) - mean[i] * mean[i]; // E[x²] − E[x]²
            let std = if var > 0.0 { var.sqrt() } else { 0.0 };
            if std > MAX_STD_DPS {
                return Err(Error::MotionDetected(std));
            }
        }

        self.gyro_bias = mean;
        Ok(mean)
    }
    pub fn release(self) -> (SPI, CS) {
        (self.spi, self.cs)
    }
}
