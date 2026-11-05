//! The local-time clock for Insert > Date and Time (TX-13). The engine only knows UTC; the
//! app knows the OS time zone, so it installs this clock.

use chrono::{Datelike, NaiveDateTime, Timelike};
use notecraft_engine::stamp::{Clock, Stamp};

/// A clock that reads the machine's local time.
pub fn local() -> Clock {
    Clock::new(|| stamp_of(&chrono::Local::now().naive_local()))
}

fn stamp_of(time: &NaiveDateTime) -> Option<Stamp> {
    Stamp::new(
        time.year(),
        u8::try_from(time.month()).ok()?,
        u8::try_from(time.day()).ok()?,
        u8::try_from(time.hour()).ok()?,
        u8::try_from(time.minute()).ok()?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    fn local_fields_become_a_stamp() {
        let time = chrono::NaiveDate::from_ymd_opt(2026, 11, 5)
            .and_then(|d| d.and_hms_opt(15, 7, 59))
            .expect("valid time");
        let stamp = stamp_of(&time).expect("stamp");
        assert_eq!(stamp.date_time(), "11/5/2026 3:07 PM");
    }

    #[test]
    fn the_local_clock_reads_a_time() {
        assert!(local().now().is_some());
    }
}
