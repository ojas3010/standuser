//! Clean activity timeline and the date stats (Speed, Stamina). Spec: "Letterboxd JoJo Stand stats 2", clean timeline.
//!
//! A bulk day (join day, >= 14 films, or >= 8 films with fewer than half corroborated) is dropped. Each film gives
//! at most one event, and rewatches are ignored.

use crate::dates::{month_label, month_of, Day};
use crate::parse::Export;
use serde::Serialize;
use std::collections::HashMap;

pub const BULK_N: i64 = 8;
pub const BULK_CAP: i64 = 14;
const LIVE: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum Source {
    Diary,
    Mark,
}

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    /// Index into Export::watched.
    pub film: usize,
    pub day: Day,
    pub src: Source,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Timeline {
    pub joined: Day,
    pub as_of: Day,
    pub events: Vec<Event>,
    pub bulk_mark_days: Vec<Day>,
    pub rows_on_bulk_mark_days: usize,
    pub dropped_bulk_mark: usize,
    pub dropped_no_clean_view: usize,
    pub metrics: Option<DateMetrics>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DateMetrics {
    /// Complete months strictly after the month of T0 and strictly before the month of as_of.
    pub months: i64,
    pub first_month: i64,
    pub events_in_window: usize,
    pub active: usize,
    pub share: f64,
    pub pace: f64,
    pub streak: usize,
    pub peak_n: usize,
    pub peak_month: String,
    pub peak_month_index: i64,
    pub silent: usize,
    pub silent_run: usize,
    pub start: String,
    /// Month index of the last month of the longest active streak.
    pub streak_end_month: i64,
    /// Events per window month (index 0 = first_month).
    pub counts: Vec<usize>,
}

fn key(name: &str, year: i32) -> (String, i32) {
    (name.to_string(), year)
}

/// Join day: Date Joined when the export has it, else the earliest watched.csv date.
pub fn effective_join(ex: &Export) -> Day {
    ex.joined.unwrap_or_else(|| ex.watched.iter().map(|w| w.day).min().unwrap_or(0))
}

/// Export day: from the file name, else the latest date in the export.
pub fn effective_as_of(ex: &Export) -> Day {
    ex.as_of.unwrap_or_else(|| {
        let a = ex.watched.iter().map(|w| w.day).max().unwrap_or(0);
        let b = ex.diary.iter().map(|d| d.log_day).max().unwrap_or(0);
        a.max(b)
    })
}

pub fn build(ex: &Export, joined: Day, as_of: Day) -> Timeline {
    // diary entries per (Name, Year): (log day, view day)
    let mut entries: HashMap<(String, i32), Vec<(Day, Day)>> = HashMap::new();
    for d in &ex.diary {
        entries.entry(key(&d.name, d.year)).or_default().push((d.log_day, d.view_day));
    }
    // mark-day counts and diary corroboration
    let mut c_w: HashMap<Day, i64> = HashMap::new();
    let mut k_w: HashMap<Day, i64> = HashMap::new();
    for w in &ex.watched {
        *c_w.entry(w.day).or_default() += 1;
        if let Some(es) = entries.get(&key(&w.name, w.year)) {
            if es.iter().any(|&(_, v)| (v - w.day).abs() <= LIVE) {
                *k_w.entry(w.day).or_default() += 1;
            }
        }
    }
    let mark_bulk = |d: Day| -> bool {
        let c = *c_w.get(&d).unwrap_or(&0);
        let k = *k_w.get(&d).unwrap_or(&0);
        if d == joined && 2 * k < c {
            return true;
        }
        c >= BULK_CAP || (c >= BULK_N && 2 * k < c)
    };
    // viewing-day counts and live-logged share
    let mut c_v: HashMap<Day, i64> = HashMap::new();
    let mut live_v: HashMap<Day, i64> = HashMap::new();
    for es in entries.values() {
        for &(l, v) in es {
            *c_v.entry(v).or_default() += 1;
            if (l - v).abs() <= LIVE {
                *live_v.entry(v).or_default() += 1;
            }
        }
    }
    let view_bulk = |d: Day| -> bool {
        let c = *c_v.get(&d).unwrap_or(&0);
        let l = *live_v.get(&d).unwrap_or(&0);
        c >= BULK_CAP || (c >= BULK_N && 2 * l < c)
    };

    let mut tl = Timeline { joined, as_of, ..Default::default() };
    for (i, w) in ex.watched.iter().enumerate() {
        if let Some(es) = entries.get(&key(&w.name, w.year)) {
            let best = es.iter().map(|&(_, v)| v).filter(|&v| v >= joined && !view_bulk(v)).min();
            match best {
                Some(v) => tl.events.push(Event { film: i, day: v, src: Source::Diary }),
                None => tl.dropped_no_clean_view += 1,
            }
        } else if mark_bulk(w.day) {
            tl.dropped_bulk_mark += 1;
        } else {
            tl.events.push(Event { film: i, day: w.day, src: Source::Mark });
        }
    }
    let mut bulk: Vec<Day> = c_w.keys().copied().filter(|&d| mark_bulk(d)).collect();
    bulk.sort();
    tl.rows_on_bulk_mark_days = bulk.iter().map(|d| c_w[d] as usize).sum();
    tl.bulk_mark_days = bulk;
    tl.metrics = metrics(&tl.events, as_of);
    tl
}

pub fn metrics(events: &[Event], as_of: Day) -> Option<DateMetrics> {
    let t0 = events.iter().map(|e| e.day).min()?;
    let m0 = month_of(t0) + 1;
    let m1 = month_of(as_of) - 1;
    if m1 < m0 {
        return None;
    }
    let l = (m1 - m0 + 1) as usize;
    let mut counts = vec![0usize; l];
    for e in events {
        let m = month_of(e.day);
        if m >= m0 && m <= m1 {
            counts[(m - m0) as usize] += 1;
        }
    }
    let n_win: usize = counts.iter().sum();
    let active = counts.iter().filter(|&&c| c > 0).count();
    // longest active streak (first of equal length) and longest silent run
    let (mut streak, mut cur, mut streak_end) = (0usize, 0usize, 0usize);
    let (mut silent_run, mut srun) = (0usize, 0usize);
    for (i, &c) in counts.iter().enumerate() {
        if c > 0 {
            cur += 1;
            srun = 0;
            if cur > streak {
                streak = cur;
                streak_end = i;
            }
        } else {
            cur = 0;
            srun += 1;
            silent_run = silent_run.max(srun);
        }
    }
    let (mut peak_i, mut peak_n) = (0usize, 0usize);
    for (i, &c) in counts.iter().enumerate() {
        if c > peak_n {
            peak_n = c;
            peak_i = i;
        }
    }
    Some(DateMetrics {
        months: l as i64,
        first_month: m0,
        events_in_window: n_win,
        active,
        share: active as f64 / l as f64,
        pace: if active > 0 { n_win as f64 / active as f64 } else { 0.0 },
        streak,
        peak_n,
        peak_month: month_label(m0 + peak_i as i64),
        peak_month_index: m0 + peak_i as i64,
        silent: l - active,
        silent_run,
        start: month_label(m0),
        streak_end_month: m0 + streak_end as i64,
        counts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::parse_date;
    use crate::parse::{DiaryEntry, Watched};

    fn d(s: &str) -> Day {
        parse_date(s).unwrap()
    }
    fn w(day: &str, n: &str) -> Watched {
        Watched { day: d(day), name: n.into(), year: 2000, uri: String::new() }
    }

    #[test]
    fn join_day_and_bulk_marks_dropped() {
        let mut ex = Export::default();
        // 10 films marked on the join day, 3 films on ordinary days
        for i in 0..10 {
            ex.watched.push(w("2022-07-06", &format!("J{i}")));
        }
        ex.watched.push(w("2022-08-10", "A"));
        ex.watched.push(w("2022-09-10", "B"));
        ex.watched.push(w("2022-10-10", "C"));
        let tl = build(&ex, d("2022-07-06"), d("2022-12-20"));
        assert_eq!(tl.events.len(), 3);
        assert_eq!(tl.dropped_bulk_mark, 10);
        assert_eq!(tl.rows_on_bulk_mark_days, 10);
        let m = tl.metrics.unwrap();
        // T0 = Aug 2022 -> window Sep..Nov 2022 = 3 months; B and C inside, A is T0's month
        assert_eq!(m.months, 3);
        assert_eq!(m.events_in_window, 2);
        assert_eq!(m.active, 2);
        assert_eq!(m.silent, 1);
    }

    #[test]
    fn diary_view_date_wins_and_pre_join_views_ignored() {
        let mut ex = Export::default();
        ex.watched.push(w("2022-08-01", "A"));
        ex.diary.push(DiaryEntry { log_day: d("2022-08-01"), name: "A".into(), year: 2000, view_day: d("2022-07-20"), rewatch: false });
        ex.diary.push(DiaryEntry { log_day: d("2022-08-01"), name: "A".into(), year: 2000, view_day: d("2022-07-30"), rewatch: false });
        let tl = build(&ex, d("2022-07-25"), d("2022-12-20"));
        assert_eq!(tl.events.len(), 1);
        assert_eq!(tl.events[0].day, d("2022-07-30"), "earliest view on or after the join day");
        assert_eq!(tl.events[0].src, Source::Diary);
    }

    #[test]
    fn streak_and_silence() {
        let evs: Vec<Event> = ["2022-01-15", "2022-02-15", "2022-03-15", "2022-06-15", "2022-07-15", "2022-12-31"]
            .iter()
            .enumerate()
            .map(|(i, s)| Event { film: i, day: d(s), src: Source::Mark })
            .collect();
        // T0 = Jan 2022; as_of Oct 2022 -> window Feb..Sep (8 months): Feb, Mar, Jun, Jul active
        let m = metrics(&evs, d("2022-10-05")).unwrap();
        assert_eq!((m.months, m.active, m.streak, m.silent, m.silent_run), (8, 4, 2, 4, 2));
        assert_eq!(m.start, "Feb 2022");
        assert_eq!(m.peak_n, 1);
    }

    #[test]
    fn no_window_no_metrics() {
        let evs = vec![Event { film: 0, day: d("2022-10-01"), src: Source::Mark }];
        assert!(metrics(&evs, d("2022-11-15")).is_none());
    }
}
