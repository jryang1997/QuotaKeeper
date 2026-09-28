//! Forecast only quota windows with a known duration, reset time, and enough observations.

use chrono::{Datelike, Local, TimeZone};
use tui_token_monitor::model::Limit;

#[derive(Clone, Copy, Debug)]
pub struct Forecast {
    pub delta_points: f64,
    pub exhaust_in_ms: Option<i64>,
}

fn window_ms(label: &str, now_ms: i64) -> Option<i64> {
    let label = label.to_lowercase();
    if label.contains("5h")
        || label.contains("5 h")
        || label.contains("5 hour")
        || label.contains("5小时")
    {
        return Some(5 * 3_600_000);
    }
    if label.contains("7d")
        || label.contains("7 d")
        || label.contains("7 day")
        || label.contains("week")
        || label.contains('周')
    {
        return Some(7 * 86_400_000);
    }
    if label.contains("month") || label.contains('月') {
        let now = Local.timestamp_millis_opt(now_ms).single()?;
        let first = Local
            .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
            .single()?;
        let (year, month) = if now.month() == 12 {
            (now.year() + 1, 1)
        } else {
            (now.year(), now.month() + 1)
        };
        let next = Local.with_ymd_and_hms(year, month, 1, 0, 0, 0).single()?;
        return Some(next.timestamp_millis() - first.timestamp_millis());
    }
    None
}

pub fn forecast(limit: &Limit, now_ms: i64) -> Option<Forecast> {
    let total = window_ms(&limit.label, now_ms)?;
    let reset = limit.resets_in_ms?;
    let used = limit.used_pct;
    if reset <= 0 || reset >= total || !(5.0..100.0).contains(&used) {
        return None;
    }
    let elapsed = total - reset;
    if elapsed < total / 10 {
        return None;
    }
    let eta = (elapsed as f64 * (100.0 - used) / used) as i64;
    if eta <= 0 {
        return None;
    }
    Some(Forecast {
        delta_points: used - elapsed as f64 / total as f64 * 100.0,
        exhaust_in_ms: if eta < reset { Some(eta) } else { None },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tui_token_monitor::model::Status;

    #[test]
    fn five_hour_forecast_has_pace_delta_and_eta() {
        let limit = Limit {
            label: "5h".into(),
            used_pct: 80.0,
            resets_in_ms: Some(2 * 3_600_000),
            status: Status::Warning,
        };
        let result = forecast(&limit, 1_790_000_000_000).unwrap();
        assert!((result.delta_points - 20.0).abs() < 0.001);
        assert_eq!(result.exhaust_in_ms, Some(2_700_000));
    }

    #[test]
    fn weak_sample_has_no_forecast() {
        let limit = Limit {
            label: "5h".into(),
            used_pct: 2.0,
            resets_in_ms: Some(4 * 3_600_000),
            status: Status::Ok,
        };
        assert!(forecast(&limit, 1_790_000_000_000).is_none());
    }
}
