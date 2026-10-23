//! The one timestamp type past the `node` boundary.

use std::fmt;
use std::str::FromStr;

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

/// A time as a person types it, in UTC: `2026-10-22T06:00:00Z`, or a date,
/// `2026-10-22`, for its first second. Nothing before 1970.
///
/// ```
/// use selfcast_core::time::Timestamp;
/// let t: Timestamp = "2024-03-03T09:46:40Z".parse().unwrap();
/// assert_eq!(t, Timestamp::from_farcaster(100_000_000));
/// assert_eq!("2024-03-03".parse::<Timestamp>().unwrap().to_string(), "2024-03-03T00:00:00Z");
/// ```
impl FromStr for Timestamp {
    type Err = ParseTimeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (date, time) = s.split_once('T').unwrap_or((s, "00:00:00Z"));
        let time = time.strip_suffix('Z').ok_or(ParseTimeError)?;
        let [y, m, d] = fields(date, '-', [4, 2, 2])?;
        let [hh, mm, ss] = fields(time, ':', [2, 2, 2])?;
        let valid = (1970..).contains(&y)
            && (1..=12).contains(&m)
            && (1..=days_in_month(y, m)).contains(&d)
            && hh < 24
            && mm < 60
            && ss < 60;
        match valid {
            true => Ok(Self(
                days_from_civil(y, m, d) * 86_400 + hh * 3600 + mm * 60 + ss,
            )),
            false => Err(ParseTimeError),
        }
    }
}

/// Why a time typed by a person did not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a time is UTC, as 2026-10-22T06:00:00Z or 2026-10-22")]
pub struct ParseTimeError;

/// Three numbers of exactly `widths` digits, separated by `sep`.
fn fields(s: &str, sep: char, widths: [usize; 3]) -> Result<[u64; 3], ParseTimeError> {
    let parts: Vec<&str> = s.split(sep).collect();
    let ok = parts.len() == 3
        && parts
            .iter()
            .zip(widths)
            .all(|(p, w)| p.len() == w && p.bytes().all(|b| b.is_ascii_digit()));
    match ok {
        true => Ok([0, 1, 2].map(|i| parts[i].parse().expect("checked to be digits"))),
        false => Err(ParseTimeError),
    }
}

fn days_in_month(y: u64, m: u64) -> u64 {
    let leap = (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400);
    match m {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// A proleptic Gregorian date to days since 1970-01-01, for dates from
/// 1970. Howard Hinnant's `days_from_civil`, the inverse of
/// [`civil_from_days`].
fn days_from_civil(y: u64, m: u64, d: u64) -> u64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
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
    fn parsing_inverts_formatting_for_every_day_from_1970_to_2200() {
        (0..(230 * 366))
            .map(|day| Timestamp::from_unix(day * 86_400 + 86_399))
            .filter(|t| t.unix_seconds() < 7_258_118_400)
            .for_each(|t| assert_eq!(t.to_string().parse(), Ok(t), "{t}"));
    }

    #[test]
    fn malformed_times_are_refused() {
        [
            "",
            "2026-10-22T",
            "2026-10-22T06:00:00",
            "2026-1-22",
            "2026-02-30",
            "2025-02-29",
            "2026-10-22T24:00:00Z",
            "1969-12-31",
            "2026-10-22 06:00:00Z",
            "+026-10-22",
            "2026-10-22T06:00Z",
            "2100-02-29",
        ]
        .iter()
        .for_each(|s| assert!(s.parse::<Timestamp>().is_err(), "{s:?} accepted"));
        assert!("2024-02-29".parse::<Timestamp>().is_ok());
        assert!("2000-02-29".parse::<Timestamp>().is_ok());
    }

    #[test]
    fn leap_day_formats() {
        assert_eq!(
            Timestamp::from_farcaster(99_705_600).to_string(),
            "2024-02-29T00:00:00Z"
        );
    }
}
