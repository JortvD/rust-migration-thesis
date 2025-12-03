pub fn least_squares_slope(values: &[usize]) -> Option<f64> {
    let n = values.len();
    if n <= 1 {
        return None;
    }

    let mean_x = (n as f64 - 1.0) / 2.0;
    let mut mean_y = 0.0f64;
    for &v in values {
        mean_y += v as f64;
    }
    mean_y /= n as f64;

    let mut cov_xy = 0.0f64;
    let mut var_x = 0.0f64;
    for (i, &v) in values.iter().enumerate() {
        let x = i as f64;
        let y = v as f64;
        let dx = x - mean_x;
        cov_xy += dx * (y - mean_y);
        var_x += dx * dx;
    }

    if var_x.abs() < std::f64::EPSILON {
        None
    } else {
        Some(cov_xy / var_x)
    }
}