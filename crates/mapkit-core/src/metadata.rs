//! Supplied metadata only: no wall-clock, identity registry or signature checks.
use crate::{error, Attribution, Provenance, Result};
use schemars::{gen::SchemaGenerator, schema::Schema, JsonSchema};

pub(crate) fn timestamp_schema(generator: &mut SchemaGenerator) -> Schema {
    let mut schema = String::json_schema(generator).into_object();
    // Calendar validity and ordering are semantic checks, not regex assertions.
    schema.string().pattern = Some(
        concat!(
            r"^[0-9]{4}-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])[Tt]",
            r"([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9](\.[0-9]{1,9})?",
            r"([Zz]|[+-]([01][0-9]|2[0-3]):[0-5][0-9])$"
        )
        .into(),
    );
    schema.into()
}

fn label(value: &str) -> bool {
    !value.trim().is_empty() && !value.chars().any(char::is_control)
}

impl Provenance {
    pub fn validate(&self) -> Result<()> {
        for (name, value) in [
            ("tool_id", &self.tool_id),
            ("version", &self.version),
            ("build_id", &self.build_id),
            ("fingerprint", &self.fingerprint),
        ] {
            if !label(value) {
                return Err(error(
                    "E_PROVENANCE",
                    format!(
                        "provenance.{name} must be a nonblank label without control characters"
                    ),
                ));
            }
        }
        let first = timestamp(&self.first_created)
            .ok_or_else(|| error("E_PROVENANCE", "invalid provenance.first_created timestamp"))?;
        let last = timestamp(&self.last_edited)
            .ok_or_else(|| error("E_PROVENANCE", "invalid provenance.last_edited timestamp"))?;
        if last < first {
            return Err(error("E_PROVENANCE", "last_edited precedes first_created"));
        }
        Ok(())
    }
}

impl Attribution {
    pub fn validate(&self, location: &str) -> Result<()> {
        if !label(&self.source)
            || !label(&self.license)
            || self
                .notice
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            return Err(error(
                "E_ATTRIBUTION",
                format!("{location} requires source/license labels and a text notice"),
            ));
        }
        Ok(())
    }
}

// Bounded RFC 3339 profile: years 0001..9999, ordinary seconds, up to ns precision,
// known UTC offset. Return comparable UTC seconds and nanoseconds without floats.
fn timestamp(value: &str) -> Option<(i64, u32)> {
    let b = value.as_bytes();
    if !(20..=35).contains(&b.len())
        || !b.is_ascii()
        || b[4] != b'-'
        || b[7] != b'-'
        || !matches!(b[10], b'T' | b't')
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let number = |start: usize, end: usize| -> Option<i64> {
        let digits = b.get(start..end)?;
        digits
            .iter()
            .all(u8::is_ascii_digit)
            .then(|| digits.iter().fold(0, |n, d| n * 10 + i64::from(d - b'0')))
    };
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    if year == 0 || !(1..=12).contains(&month) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if day < 1 || day > months[(month - 1) as usize] {
        return None;
    }
    let mut at = 19;
    let mut nanos = 0;
    if b[at] == b'.' {
        at += 1;
        let start = at;
        while b.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
        let count = at - start;
        if !(1..=9).contains(&count) {
            return None;
        }
        nanos = number(start, at)? as u32 * 10u32.pow(9 - count as u32);
    }
    let offset = match b.get(at..)? {
        [b'Z' | b'z'] => 0,
        [sign @ (b'+' | b'-'), _, _, b':', _, _] => {
            let (h, m) = (number(at + 1, at + 3)?, number(at + 4, at + 6)?);
            if h > 23 || m > 59 || (*sign == b'-' && h == 0 && m == 0) {
                return None;
            }
            (h * 3600 + m * 60) * if *sign == b'-' { -1 } else { 1 }
        }
        _ => return None,
    };
    let y = year - 1;
    let days = y * 365 + y / 4 - y / 100
        + y / 400
        + months[..(month - 1) as usize].iter().sum::<i64>()
        + day
        - 1;
    Some((
        days * 86400 + hour * 3600 + minute * 60 + second - offset,
        nanos,
    ))
}
