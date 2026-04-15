//! Board-specific concrete type aliases for WeAct Mini STM32H743

use stm32h7xx_hal::{gpio, pac, pwm, spi};

/// SPI2 in enabled mode, 8-bit word
pub type Spi2 = spi::Spi<pac::SPI2, spi::Enabled, u8>;

/// IMU chip select: PB12 as push-pull output
pub type ImuCs = gpio::Pin<'B', 12, gpio::Output<gpio::PushPull>>;

/// Concrete ICM-42688-P driver type
pub type Imu = crate::drivers::icm42688p::Icm42688p<Spi2, ImuCs>;

/// Built-in LED: PE3 as push-pull output
pub type Led = gpio::Pin<'E', 3, gpio::Output<gpio::PushPull>>;

/// PWM channel types for TIM3 (motor ESC outputs)
pub type PwmCh1 = pwm::Pwm<pac::TIM3, 0, pwm::ComplementaryImpossible>;
pub type PwmCh2 = pwm::Pwm<pac::TIM3, 1, pwm::ComplementaryImpossible>;
pub type PwmCh3 = pwm::Pwm<pac::TIM3, 2, pwm::ComplementaryImpossible>;
pub type PwmCh4 = pwm::Pwm<pac::TIM3, 3, pwm::ComplementaryImpossible>;
