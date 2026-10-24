//! Calendar helpers. A "day" is days since 1970-01-01; a "month" is months since 1970-01.

pub type Day = i64;

pub fn days_from_civil(y: i64, m: i64, d: i64) -> Day {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil_from_days(z: Day) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// "YYYY-MM-DD" (extra time text after the date is ignored). Works on bytes, so a multi-byte character anywhere in the
/// cell gives None rather than a panic.
pub fn parse_date(s: &str) -> Option<Day> {
    let b = s.trim().as_bytes();
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let d = &b[r];
        if d.iter().all(u8::is_ascii_digit) {
            Some(d.iter().fold(0i64, |a, c| a * 10 + (c - b'0') as i64))
        } else {
            None
        }
    };
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

pub fn month_of(day: Day) -> i64 {
    let (y, m, _) = civil_from_days(day);
    (y - 1970) * 12 + (m - 1)
}

const MON: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// "Aug 2022"
pub fn month_label(month: i64) -> String {
    let y = 1970 + month.div_euclid(12);
    let m = month.rem_euclid(12) as usize;
    format!("{} {}", MON[m], y)
}

pub fn iso(day: Day) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        for d in [-1000, 0, 1, 365, 19000, 20750] {
            let (y, m, dd) = civil_from_days(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
        assert_eq!(parse_date("1970-01-01"), Some(0));
        assert_eq!(iso(parse_date("2022-07-06").unwrap()), "2022-07-06");
    }

    #[test]
    fn malformed_dates_give_none_not_a_panic() {
        for bad in ["2022-07-0é", "2022-0é-06", "é022-07-06", "２０２２-07-06", "2022/07/06", "2022-13-01", "2022-07-32", "", "2022-07", "2022-07-0\u{301}6"] {
            assert_eq!(parse_date(bad), None, "{bad:?}");
        }
        assert_eq!(parse_date(" 2022-07-06 12:00"), parse_date("2022-07-06"));
    }

    #[test]
    fn months() {
        let d = parse_date("2022-08-15").unwrap();
        assert_eq!(month_label(month_of(d)), "Aug 2022");
        assert_eq!(month_of(parse_date("1970-02-01").unwrap()), 1);
    }
}
