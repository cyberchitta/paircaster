//! The one timestamp type past the `node` boundary.

use std::fmt;

/// The Farcaster epoch, 2021-01-01T00:00:00Z, in Unix seconds.
const FARCASTER_EPOCH: u64 = 1_609_459_200;

/// A point in time, to the second, in UTC.
///
/// ```
/// use selfcast_core::time::Timestamp;
/// let t = Timestamp::from_farcaster(100_000_000);
/// assert_eq!(t.unix_seconds(), 1_709_459_200);
/// assert_eq!(t.to_string(), "2024-03-03T09:46:40Z");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(u64);

impl Timestamp {
    /// From a protocol timestamp: seconds since the Farcaster epoch.
    pub fn from_farcaster(seconds: u32) -> Self {
        Self(FARCASTER_EPOCH + u64::from(seconds))
    }

    pub const fn from_unix(seconds: u64) -> Self {
        Self(seconds)
    }

    pub fn unix_seconds(self) -> u64 {
        self.0
    }

    /// To a protocol timestamp; `None` before the Farcaster epoch or past
    /// what 32 bits hold (2157).
    pub fn to_farcaster(self) -> Option<u32> {
        self.0
            .checked_sub(FARCASTER_EPOCH)
            .and_then(|s| u32::try_from(s).ok())
    }
}

/// RFC 3339 in UTC, e.g. `2021-01-01T00:00:00Z`.
impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (days, secs) = (self.0 / 86_400, self.0 % 86_400);
        let (y, m, d) = civil_from_days(days);
        let (hh, mm, ss) = (secs / 3600, secs % 3600 / 60, secs % 60);
        write!(f, "{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
    }
}

/// Days since 1970-01-01 to a proleptic Gregorian date, for days ≥ 0.
/// Howard Hinnant's `civil_from_days` (public domain, "chrono-Compatible
/// Low-Level Date Algorithms").
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + u64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::Timestamp;

    #[test]
    fn the_farcaster_epoch_is_2021_01_01() {
        assert_eq!(
            Timestamp::from_farcaster(0).to_string(),
            "2021-01-01T00:00:00Z"
        );
    }

    #[test]
    fn formats_as_python_does() {
        assert_eq!(
            Timestamp::from_farcaster(150_000_000).to_string(),
            "2025-10-03T02:40:00Z"
        );
    }

    #[test]
    fn leap_day_formats() {
        assert_eq!(
            Timestamp::from_farcaster(99_705_600).to_string(),
            "2024-02-29T00:00:00Z"
        );
    }
}
