#[inline(always)]
pub fn clamp<T: PartialOrd>(val: T, min: T, max: T) -> T {
    if val < min {
        min
    } else if val > max {
        max
    } else {
        val
    }
}

/// Fast inverse square root
pub fn fast_inv_sqrt(x: f32) -> f32 {
    let half = 0.5 * x;
    let mut i = x.to_bits();
    i = 0x5f3759df - (i >> 1); // Evil magic number. Wtf??
    let mut y = f32::from_bits(i);
    y = y * (1.5 - half * y * y); // 1st Newton-Raphson iteration
    y = y * (1.5 - half * y * y); // 2nd iteration for better precision
    y
}

/// atan2 approximation (max error ~0.01 rad)
pub fn atan2f(y: f32, x: f32) -> f32 {
    if x == 0.0 && y == 0.0 {
        return 0.0;
    }

    let abs_x = x.abs();
    let abs_y = y.abs();

    let (a, mut result) = if abs_x > abs_y {
        (y / x, 0.0f32)
    } else {
        let a = x / y;
        (a, core::f32::consts::FRAC_PI_2)
    };

    // Polynomial approximation of atan for |a| <= 1
    let a2 = a * a;
    let atan_a = a * (1.0 - a2 * (0.3333333 - a2 * (0.2 - a2 * 0.1428571)));

    if abs_x > abs_y {
        result = atan_a;
    } else {
        result -= atan_a;
    }

    // Adjust quadrant
    if x < 0.0 {
        if y >= 0.0 {
            result += core::f32::consts::PI;
        } else {
            result -= core::f32::consts::PI;
        }
    }

    result
}

/// asin approximation (max error ~0.005 rad for |x| < 0.97)
pub fn asinf(x: f32) -> f32 {
    // Padé-style approximation
    let x = clamp(x, -1.0, 1.0);
    let x2 = x * x;
    x * (1.0 + x2 * (0.16666667 + x2 * (0.075 + x2 * 0.04464286)))
}
