//! Lines written at the pace their log was written
//!
//! Each entry waits for the time between its timestamp and the timestamp of
//! the last entry that had one, divided by the speed. An entry without a
//! timestamp, and one written before the last, waits for none.
use crate::merge::Moment;
use std::time::Duration;

const NANOS_PER_SECOND: f64 = 1_000_000_000.0;

/// How long each entry waits before it is written
pub fn delays(moments: &[Option<Moment>], speed: f64) -> Vec<Duration> {
    let mut last: Option<Moment> = None;

    moments
        .iter()
        .map(|moment| {
            let delay = match (last, *moment) {
                (Some((last_seconds, last_nanos)), Some((seconds, nanos))) => {
                    let elapsed = (seconds - last_seconds) as f64
                        + (nanos as f64 - last_nanos as f64) / NANOS_PER_SECOND;

                    Duration::from_secs_f64((elapsed / speed).max(0.0))
                }
                _ => Duration::ZERO,
            };

            last = moment.or(last);
            delay
        })
        .collect()
}

/// Reads a `--replay` speed, which must be a number above zero
pub fn parse_speed(text: &str) -> Result<f64, String> {
    match text.parse::<f64>() {
        Ok(speed) if speed > 0.0 && speed.is_finite() => Ok(speed),
        _ => Err(format!(
            "'{}' is not a speed (expected a number above zero, such as 1 or 10)",
            text
        )),
    }
}
