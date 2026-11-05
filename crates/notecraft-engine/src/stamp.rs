//! Date and time stamps for Insert > Date, Time, and Date & Time (TX-13), and the clock they
//! read.
//!
//! The format is US English by default: `11/5/2026` and `3:07 PM`. A pure-Rust, wasm-safe way
//! to read the OS locale's date and time patterns does not exist without a locale database, so
//! the format is fixed; only the clock follows the machine (see [`Clock`]).
//!
//! The clock is injectable so tests are deterministic. A [`Session`](crate::Session) starts
//! with a UTC clock (all the engine can know without the OS time zone, which L5 does not
//! read); the desktop app installs a local-time clock with `Session::set_clock`.

use std::fmt;
use std::sync::Arc;

/// A civil date and time of day to the minute, in whatever zone the clock reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub year: i32,
    /// 1 to 12.
    pub month: u8,
    /// 1 to 31.
    pub day: u8,
    /// 0 to 23.
    pub hour: u8,
    /// 0 to 59.
    pub minute: u8,
}

impl Stamp {
    /// A stamp, or `None` when a field is out of range.
    pub fn new(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Option<Self> {
        let ok = (1..=12).contains(&month)
            && (1..=31).contains(&day)
            && hour < 24
            && minute < 60
            && (0..=9999).contains(&year);
        ok.then_some(Self {
            year,
            month,
            day,
            hour,
            minute,
        })
    }

    /// `11/5/2026`: month, day, and year, with no leading zeros.
    pub fn date(&self) -> String {
        format!("{}/{}/{}", self.month, self.day, self.year)
    }

    /// `3:07 PM`: a 12-hour clock with two-digit minutes.
    pub fn time(&self) -> String {
        let hour = match self.hour % 12 {
            0 => 12,
            h => h,
        };
        let half = if self.hour < 12 { "AM" } else { "PM" };
        format!("{hour}:{:02} {half}", self.minute)
    }

    /// `11/5/2026 3:07 PM`: the date, a space, then the time.
    pub fn date_time(&self) -> String {
        format!("{} {}", self.date(), self.time())
    }

    /// The stamp for `secs` seconds after 1970-01-01 00:00:00 (UTC when the offset is 0),
    /// or `None` when it is outside years 0 to 9999.
    pub fn from_unix(secs: i64) -> Option<Self> {
        let days = secs.div_euclid(86_400);
        let rest = secs.rem_euclid(86_400);
        // Civil date from a day count (proleptic Gregorian).
        let z = days.checked_add(719_468)?;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        Self::new(
            i32::try_from(year).ok()?,
            u8::try_from(month).ok()?,
            u8::try_from(day).ok()?,
            u8::try_from(rest / 3_600).ok()?,
            u8::try_from(rest % 3_600 / 60).ok()?,
        )
    }
}

/// Where the current time comes from. Cheap to clone.
#[derive(Clone)]
pub struct Clock(Arc<dyn Fn() -> Option<Stamp> + Send + Sync>);

impl Clock {
    /// A clock that calls `now`. It returns `None` when the time is not available.
    pub fn new(now: impl Fn() -> Option<Stamp> + Send + Sync + 'static) -> Self {
        Self(Arc::new(now))
    }

    /// A clock that always reads `stamp` (for tests).
    pub fn fixed(stamp: Stamp) -> Self {
        Self::new(move || Some(stamp))
    }

    /// The UTC time from the system clock. Not available in a browser build, where the
    /// system clock is not readable from the engine.
    pub fn utc() -> Self {
        Self::new(utc_now)
    }

    /// The time now, if it can be read.
    pub fn now(&self) -> Option<Stamp> {
        (self.0)()
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::utc()
    }
}

impl fmt::Debug for Clock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Clock")
    }
}

/// Clocks are behavior, not state: two sessions are equal whatever clock they read.
impl PartialEq for Clock {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn utc_now() -> Option<Stamp> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Stamp::from_unix(i64::try_from(secs).ok()?)
}

#[cfg(target_arch = "wasm32")]
fn utc_now() -> Option<Stamp> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(y: i32, mo: u8, d: u8, h: u8, mi: u8) -> Stamp {
        Stamp::new(y, mo, d, h, mi).expect("valid stamp")
    }

    #[test]
    fn formats_are_us_english() {
        let s = stamp(2026, 11, 5, 15, 7);
        assert_eq!(s.date(), "11/5/2026");
        assert_eq!(s.time(), "3:07 PM");
        assert_eq!(s.date_time(), "11/5/2026 3:07 PM");
    }

    #[test]
    fn the_twelve_hour_clock_wraps_at_midnight_and_noon() {
        assert_eq!(stamp(2026, 1, 1, 0, 0).time(), "12:00 AM");
        assert_eq!(stamp(2026, 1, 1, 0, 59).time(), "12:59 AM");
        assert_eq!(stamp(2026, 1, 1, 11, 59).time(), "11:59 AM");
        assert_eq!(stamp(2026, 1, 1, 12, 0).time(), "12:00 PM");
        assert_eq!(stamp(2026, 1, 1, 23, 5).time(), "11:05 PM");
        assert_eq!(stamp(2026, 12, 31, 9, 5).date(), "12/31/2026");
    }

    #[test]
    fn unix_seconds_become_civil_time() {
        assert_eq!(Stamp::from_unix(0), Some(stamp(1970, 1, 1, 0, 0)));
        assert_eq!(
            Stamp::from_unix(951_782_400),
            Some(stamp(2000, 2, 29, 0, 0))
        );
        assert_eq!(
            Stamp::from_unix(1_793_891_220),
            Some(stamp(2026, 11, 5, 15, 7))
        );
        assert_eq!(Stamp::from_unix(-1), Some(stamp(1969, 12, 31, 23, 59)));
        assert_eq!(Stamp::from_unix(i64::MAX), None);
        assert_eq!(Stamp::from_unix(i64::MIN), None);
    }

    #[test]
    fn out_of_range_fields_are_refused() {
        assert_eq!(Stamp::new(2026, 0, 1, 0, 0), None);
        assert_eq!(Stamp::new(2026, 13, 1, 0, 0), None);
        assert_eq!(Stamp::new(2026, 1, 32, 0, 0), None);
        assert_eq!(Stamp::new(2026, 1, 1, 24, 0), None);
        assert_eq!(Stamp::new(2026, 1, 1, 0, 60), None);
        assert_eq!(Stamp::new(-1, 1, 1, 0, 0), None);
    }

    #[test]
    fn clocks_can_be_fixed_or_missing() {
        let s = stamp(2026, 11, 5, 15, 7);
        assert_eq!(Clock::fixed(s).now(), Some(s));
        assert_eq!(Clock::new(|| None).now(), None);
        #[cfg(not(target_arch = "wasm32"))]
        assert!(Clock::utc().now().is_some());
    }
}
