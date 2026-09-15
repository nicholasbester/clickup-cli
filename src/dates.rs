//! Parsing of user-supplied due dates into ClickUp's Unix-millisecond form.
//!
//! ClickUp stores a due date as an instant, but a date-only due date (no
//! `due_date_time`) is displayed and snapped to 04:00 in the *workspace*
//! timezone of whatever calendar day that instant falls on. Interpreting
//! `YYYY-MM-DD` as midnight UTC therefore lands on the previous day for every
//! user west of UTC (GH #126). To stay inside the intended day for any
//! reasonable offset between the machine and the workspace, a date-only value
//! is anchored at **local noon** rather than local midnight: noon also
//! sidesteps DST transitions where local midnight does not exist or occurs
//! twice.
//!
//! Accepted forms, tried in order:
//!
//! | input                          | interpretation                     | `due_date_time` |
//! |--------------------------------|------------------------------------|-----------------|
//! | `1798693200000`                | Unix ms (>= 12 digits), passed through | omitted     |
//! | `2026-12-31`                   | local noon on that day             | omitted         |
//! | `2026-12-31T09:30[:00]`        | that wall-clock time, local zone   | `true`          |
//! | `2026-12-31T09:30[:00]Z`       | that instant                       | `true`          |
//! | `2026-12-31T09:30[:00]±HH:MM`  | that instant                       | `true`          |
//!
//! A space may replace the `T` separator.

use crate::error::CliError;
use chrono::{DateTime, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};

/// A parsed due date ready to be placed in a request body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DueDate {
    /// Unix timestamp in milliseconds.
    pub ms: i64,
    /// Whether the user supplied a time-of-day. When true the request should
    /// carry `due_date_time: true` so ClickUp keeps the exact instant instead
    /// of snapping to its date-only convention.
    pub has_time: bool,
}

impl DueDate {
    /// Insert `due_date` (and `due_date_time` when a time was given) into a
    /// JSON object body.
    pub fn apply_to(&self, body: &mut serde_json::Map<String, serde_json::Value>) {
        body.insert("due_date".into(), serde_json::json!(self.ms));
        if self.has_time {
            body.insert("due_date_time".into(), serde_json::Value::Bool(true));
        }
    }
}

/// Parse a due date using the machine's local timezone for naive inputs.
pub fn parse_due_date(input: &str) -> Result<DueDate, CliError> {
    parse_due_date_in(input, &Local)
}

/// Parse a due date, resolving naive (offset-less) inputs in `tz`.
///
/// Exposed separately so tests can pin a fixed offset without depending on the
/// host timezone.
pub fn parse_due_date_in<Tz: TimeZone>(input: &str, tz: &Tz) -> Result<DueDate, CliError> {
    let s = input.trim();

    // Only a full-width millisecond timestamp is taken as a raw instant. A
    // shorter all-digit value is far more likely a compact date (`20261231`)
    // or a seconds-based timestamp than a genuine 1970 instant, and reading
    // one as milliseconds would be a silent wrong value of exactly the kind
    // #126 was. Anything shorter falls through to the format error.
    const MS_DIGITS: usize = 12; // 12 digits ≈ 2001-09 onwards
    if s.len() >= MS_DIGITS && s.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(ms) = s.parse::<i64>() {
            return Ok(DueDate {
                ms,
                has_time: false,
            });
        }
    }

    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let noon = date.and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap());
        let ms = resolve_local(tz, noon, input)?;
        return Ok(DueDate {
            ms,
            has_time: false,
        });
    }

    for fmt in [
        "%Y-%m-%dT%H:%M:%S%:z",
        "%Y-%m-%dT%H:%M%:z",
        "%Y-%m-%d %H:%M:%S%:z",
        "%Y-%m-%d %H:%M%:z",
    ] {
        if let Ok(dt) = DateTime::<FixedOffset>::parse_from_str(s, fmt) {
            return Ok(DueDate {
                ms: dt.timestamp_millis(),
                has_time: true,
            });
        }
    }
    // `%:z` does not accept a literal `Z`; handle the UTC designator by hand.
    if let Some(stripped) = s.strip_suffix('Z') {
        for fmt in [
            "%Y-%m-%dT%H:%M:%S",
            "%Y-%m-%dT%H:%M",
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d %H:%M",
        ] {
            if let Ok(naive) = NaiveDateTime::parse_from_str(stripped, fmt) {
                return Ok(DueDate {
                    ms: naive.and_utc().timestamp_millis(),
                    has_time: true,
                });
            }
        }
    }

    for fmt in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, fmt) {
            let ms = resolve_local(tz, naive, input)?;
            return Ok(DueDate { ms, has_time: true });
        }
    }

    Err(invalid(input))
}

fn resolve_local<Tz: TimeZone>(
    tz: &Tz,
    naive: NaiveDateTime,
    input: &str,
) -> Result<i64, CliError> {
    // `earliest()` picks the first of two candidates in a DST fold and returns
    // None only when the wall-clock time does not exist (a DST gap).
    tz.from_local_datetime(&naive)
        .earliest()
        .map(|dt| dt.timestamp_millis())
        .ok_or_else(|| CliError::ClientError {
            message: format!(
                "Invalid date '{}': that local time does not exist (daylight-saving gap). \
                 Pick another time or add an explicit offset, e.g. {}Z.",
                input,
                naive.format("%Y-%m-%dT%H:%M")
            ),
            status: 0,
        })
}

fn invalid(input: &str) -> CliError {
    CliError::ClientError {
        message: format!(
            "Invalid date '{}'. Use YYYY-MM-DD (local day), YYYY-MM-DDTHH:MM[:SS] (local time), \
             YYYY-MM-DDTHH:MM[:SS]Z or ±HH:MM (exact instant), or a Unix millisecond \
             timestamp (12+ digits).",
            input
        ),
        status: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, Utc};

    /// UTC-5, New York standard time. `2026-12-31T00:00:00-05:00` is
    /// 1_798_693_200_000 ms (the value quoted in GH #126).
    fn new_york_std() -> FixedOffset {
        FixedOffset::west_opt(5 * 3600).unwrap()
    }
    const NY_MIDNIGHT_2026_12_31: i64 = 1_798_693_200_000;

    #[test]
    fn date_only_resolves_to_local_noon() {
        let d = parse_due_date_in("2026-12-31", &new_york_std()).unwrap();
        assert_eq!(d.ms, NY_MIDNIGHT_2026_12_31 + 12 * 3_600_000);
        assert!(!d.has_time);
    }

    #[test]
    fn date_only_east_of_utc_stays_on_the_requested_day() {
        // UTC+13 (Tonga / NZ summer): noon local is 23:00 the previous day UTC,
        // but the day in the *local* zone is still 2026-12-31.
        let tz = FixedOffset::east_opt(13 * 3600).unwrap();
        let d = parse_due_date_in("2026-12-31", &tz).unwrap();
        let local = tz.timestamp_millis_opt(d.ms).unwrap();
        assert_eq!(
            local.format("%Y-%m-%d %H:%M").to_string(),
            "2026-12-31 12:00"
        );
    }

    #[test]
    fn naive_datetime_uses_supplied_zone_and_flags_time() {
        let d = parse_due_date_in("2026-12-31T09:30", &new_york_std()).unwrap();
        assert_eq!(d.ms, NY_MIDNIGHT_2026_12_31 + (9 * 3600 + 30 * 60) * 1000);
        assert!(d.has_time);
        let with_secs = parse_due_date_in("2026-12-31T09:30:00", &new_york_std()).unwrap();
        assert_eq!(with_secs, d);
        let with_space = parse_due_date_in("2026-12-31 09:30", &new_york_std()).unwrap();
        assert_eq!(with_space, d);
    }

    #[test]
    fn zulu_suffix_is_utc_regardless_of_zone() {
        let d = parse_due_date_in("2026-12-31T12:00:00Z", &new_york_std()).unwrap();
        assert_eq!(d.ms, 1_798_718_400_000);
        assert!(d.has_time);
        assert_eq!(
            parse_due_date_in("2026-12-31T12:00Z", &new_york_std()).unwrap(),
            d
        );
    }

    #[test]
    fn explicit_offset_is_honoured() {
        let d = parse_due_date_in("2026-12-31T00:00:00-05:00", &Utc).unwrap();
        assert_eq!(d.ms, NY_MIDNIGHT_2026_12_31);
        assert!(d.has_time);
        let plus = parse_due_date_in("2026-12-31T06:00+01:00", &Utc).unwrap();
        assert_eq!(plus.ms, NY_MIDNIGHT_2026_12_31); // 05:00Z == 00:00-05:00
    }

    #[test]
    fn compact_yyyymmdd_is_rejected_not_read_as_milliseconds() {
        // `20261231` is a plausible typo for a compact date. Read as Unix ms it
        // is 1970-08-23, a silent wrong value of exactly the kind #126 was.
        for bad in ["20261231", "261231", "1798693200"] {
            let err = parse_due_date_in(bad, &Utc).unwrap_err();
            assert!(
                err.to_string().contains("Invalid date"),
                "{bad} should be rejected, got {err}"
            );
        }
    }

    #[test]
    fn raw_unix_ms_passes_through() {
        let d = parse_due_date_in("1798693200000", &new_york_std()).unwrap();
        assert_eq!(d.ms, 1_798_693_200_000);
        assert!(!d.has_time);
    }

    #[test]
    fn surrounding_whitespace_is_tolerated() {
        let d = parse_due_date_in(" 2026-12-31 ", &new_york_std()).unwrap();
        assert_eq!(d.ms, NY_MIDNIGHT_2026_12_31 + 12 * 3_600_000);
    }

    #[test]
    fn rejects_unknown_formats_with_guidance() {
        for bad in [
            "31/12/2026",
            "2026-13-01",
            "2026-12-31T25:00",
            "tomorrow",
            "",
            "12345abc",
        ] {
            let err = parse_due_date_in(bad, &Utc).unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains("Invalid date"), "{bad}: {msg}");
            assert!(msg.contains("YYYY-MM-DD"), "{bad}: {msg}");
        }
    }

    #[test]
    fn apply_to_sets_due_date_time_only_when_time_given() {
        let mut body = serde_json::Map::new();
        DueDate {
            ms: 1,
            has_time: false,
        }
        .apply_to(&mut body);
        assert_eq!(body.get("due_date"), Some(&serde_json::json!(1)));
        assert!(body.get("due_date_time").is_none());

        let mut body = serde_json::Map::new();
        DueDate {
            ms: 2,
            has_time: true,
        }
        .apply_to(&mut body);
        assert_eq!(body.get("due_date_time"), Some(&serde_json::json!(true)));
    }
}
