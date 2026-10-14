use super::*;

/// Halve the rejected nominal duration without exceeding either stability
/// limit. Clock selection still owns the nominal and encoded time balance.
pub(super) fn refined_selection(
    selected: GpuCflSelection,
    rejected_nominal_dt_s: f64,
) -> Result<GpuCflSelection, String> {
    let cap_s = rejected_nominal_dt_s * 0.5;
    let mut dt_s = cap_s as f32;
    if f64::from(dt_s) > cap_s {
        dt_s = f32::from_bits(dt_s.to_bits() - 1);
    }
    if !dt_s.is_finite() || dt_s <= 0.0 || f64::from(dt_s) > cap_s {
        return Err("GPU refined candidate duration cannot be represented".into());
    }
    Ok(GpuCflSelection {
        dt_s,
        max_cfl_dt_s: Some(selected.max_cfl_dt_s.unwrap_or(f64::INFINITY).min(cap_s)),
        max_diffusion_dt_s: selected.max_diffusion_dt_s,
    })
}

/// The host clock keeps the nominal f64 outer duration. WGSL receives f32 dt.
/// On an uncapped final substep, choose the nearest stable f32 encoding while
/// carrying the prior encoding error, so that shader time does not drift in
/// one direction over many ticks. A cap near the outer duration uses two
/// balanced steps instead of leaving a tiny final step. The accepted clock
/// always sums the nominal durations; `encoded_time_s` records the actual
/// values sent to WGSL.
pub(super) fn select_clock_durations(
    remaining_s: f64,
    selected: GpuCflSelection,
    encoding_lag_s: f64,
) -> Result<(f64, f32), String> {
    if !encoding_lag_s.is_finite() {
        return Err("GPU encoded-time difference is invalid".into());
    }
    let stability_limit_s = selected
        .max_cfl_dt_s
        .unwrap_or(f64::INFINITY)
        .min(selected.max_diffusion_dt_s.unwrap_or(f64::INFINITY));
    let completes_outer = remaining_s <= stability_limit_s;
    let mut nominal_dt_s = if completes_outer {
        remaining_s
    } else {
        f64::from(selected.dt_s)
    };
    let mut encoded_dt_s = selected.dt_s;
    if completes_outer {
        let desired = nominal_dt_s + encoding_lag_s;
        let nearest = desired as f32;
        let per_step_error_limit = nominal_dt_s * 8.0 * f32::EPSILON as f64;
        if nearest.is_finite()
            && nearest > 0.0
            && f64::from(nearest) <= stability_limit_s
            && (f64::from(nearest) - nominal_dt_s).abs() <= per_step_error_limit
        {
            encoded_dt_s = nearest;
        } else if nearest.is_finite()
            && f64::from(nearest) > stability_limit_s
            && f64::from(selected.dt_s) < remaining_s
        {
            // A cap exactly at the nominal boundary cannot use the nearest
            // f32 encoding. Accept the safe lower step and carry the small
            // remainder into another bounded substep rather than lose time.
            nominal_dt_s = f64::from(selected.dt_s);
        }
    }
    if nominal_dt_s < remaining_s && f64::from(selected.dt_s) >= remaining_s * 0.5 {
        let half = (remaining_s * 0.5) as f32;
        if half.is_finite()
            && half > 0.0
            && f64::from(half) < remaining_s
            && f64::from(half) <= stability_limit_s
        {
            nominal_dt_s = f64::from(half);
            encoded_dt_s = half;
        }
    }
    if !nominal_dt_s.is_finite()
        || nominal_dt_s <= 0.0
        || nominal_dt_s > remaining_s
        || !encoded_dt_s.is_finite()
        || encoded_dt_s <= 0.0
        || f64::from(encoded_dt_s) > stability_limit_s
    {
        return Err("GPU selected time step violates clock or stability bound".into());
    }
    Ok((nominal_dt_s, encoded_dt_s))
}
