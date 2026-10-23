//! Minimal UTC timestamp formatting (RFC 3339 and a sortable compact form)
//! so the store lane needs no date/time dependency.

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stamp {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    nanos: u32,
}

impl Stamp {
    pub(crate) fn now() -> Self {
        let d = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Self::from_unix(
            i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
            d.subsec_nanos(),
        )
    }

    pub(crate) fn from_unix(secs: i64, nanos: u32) -> Self {
        let days = secs.div_euclid(86_400);
        let rem = u32::try_from(secs.rem_euclid(86_400)).unwrap_or(0);
        // Howard Hinnant's civil-from-days algorithm.
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
        let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
        let year = yoe + era * 400 + i64::from(month <= 2);
        Self {
            year,
            month,
            day,
            hour: rem / 3600,
            minute: rem % 3600 / 60,
            second: rem % 60,
            nanos,
        }
    }

    /// `2026-10-23T10:15:00Z`
    pub(crate) fn rfc3339(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }

    /// `20261023T101500.123456789Z`: fixed width, sorts chronologically, safe in file names.
    pub(crate) fn compact(&self) -> String {
        format!(
            "{:04}{:02}{:02}T{:02}{:02}{:02}.{:09}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second, self.nanos
        )
    }
}

pub(crate) fn now_rfc3339() -> String {
    Stamp::now().rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_instants() {
        assert_eq!(Stamp::from_unix(0, 0).rfc3339(), "1970-01-01T00:00:00Z");
        assert_eq!(
            Stamp::from_unix(1_700_000_000, 5).rfc3339(),
            "2023-11-14T22:13:20Z"
        );
        assert_eq!(
            Stamp::from_unix(1_709_164_800, 0).rfc3339(),
            "2024-02-29T00:00:00Z"
        );
        assert_eq!(
            Stamp::from_unix(1_700_000_000, 5).compact(),
            "20231114T221320.000000005Z"
        );
    }
}
