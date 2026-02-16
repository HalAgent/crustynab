use chrono::{Datelike, NaiveDate};
use crustynab::calendar_weeks::{
    month_week_for_date, month_weeks, partition_year_into_month_weeks,
};

#[test]
fn partition_2024_first_week_starts_jan_1() {
    let weeks = partition_year_into_month_weeks(2024);
    let first = &weeks[0];
    insta::assert_snapshot!(format!(
        "month={} start={} end={} week_number={}",
        first.month, first.week_start, first.week_end, first.week_number
    ));
}

#[test]
fn partition_2024_covers_all_days() {
    let weeks = partition_year_into_month_weeks(2024);
    let mut all_dates: Vec<NaiveDate> = weeks.iter().flat_map(|w| w.dates()).collect();
    all_dates.sort();
    all_dates.dedup();
    let first = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let last = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
    let expected_count = (last - first).num_days() + 1;
    insta::assert_snapshot!(format!(
        "count={} first={} last={}",
        all_dates.len(),
        all_dates.first().unwrap(),
        all_dates.last().unwrap()
    ));
    assert_eq!(all_dates.len() as i64, expected_count);
}

#[test]
fn partition_2024_no_duplicate_dates() {
    let weeks = partition_year_into_month_weeks(2024);
    let all_dates: Vec<NaiveDate> = weeks.iter().flat_map(|w| w.dates()).collect();
    let mut sorted = all_dates.clone();
    sorted.sort();
    sorted.dedup();
    insta::assert_snapshot!(format!("total={} unique={}", all_dates.len(), sorted.len()));
    assert_eq!(all_dates.len(), sorted.len());
}

#[test]
fn partition_2024_week_boundaries() {
    let weeks = partition_year_into_month_weeks(2024);
    let mut issues = Vec::new();
    for w in &weeks {
        let start_dow = w.week_start.weekday();
        let end_dow = w.week_end.weekday();
        let month_first = NaiveDate::from_ymd_opt(2024, w.month, 1).unwrap();
        let start_ok = start_dow == chrono::Weekday::Sun || w.week_start == month_first;
        let month_last_day = if w.month == 12 {
            NaiveDate::from_ymd_opt(2025, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(2024, w.month + 1, 1)
        }
        .unwrap()
        .pred_opt()
        .unwrap();
        let end_ok = end_dow == chrono::Weekday::Sat || w.week_end == month_last_day;
        if !start_ok || !end_ok {
            issues.push(format!(
                "week {} month {} start={} ({:?}) end={} ({:?})",
                w.week_number, w.month, w.week_start, start_dow, w.week_end, end_dow
            ));
        }
    }
    insta::assert_snapshot!(format!("issues_count={}", issues.len()));
    assert!(issues.is_empty(), "boundary violations: {issues:?}");
}

#[test]
fn month_weeks_march_2024() {
    let weeks = month_weeks(2024, 3);
    let summary: Vec<String> = weeks
        .iter()
        .map(|w| format!("week {} {}-{}", w.week_number, w.week_start, w.week_end))
        .collect();
    insta::assert_snapshot!(summary.join("\n"));
}

#[test]
fn month_week_for_date_2024_03_13() {
    let day = NaiveDate::from_ymd_opt(2024, 3, 13).unwrap();
    let w = month_week_for_date(day).unwrap();
    insta::assert_snapshot!(format!(
        "month={} start={} end={} week_number={}",
        w.month, w.week_start, w.week_end, w.week_number
    ));
}

#[test]
fn month_week_for_date_contains_date() {
    let test_dates = vec![
        NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        NaiveDate::from_ymd_opt(2024, 2, 29).unwrap(),
        NaiveDate::from_ymd_opt(2024, 6, 15).unwrap(),
        NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        NaiveDate::from_ymd_opt(2026, 2, 14).unwrap(),
    ];
    let results: Vec<String> = test_dates
        .iter()
        .map(|&d| {
            let w = month_week_for_date(d).unwrap();
            let contains = w.week_start <= d && d <= w.week_end;
            format!(
                "{d} contained={contains} week={}-{}",
                w.week_start, w.week_end
            )
        })
        .collect();
    insta::assert_snapshot!(results.join("\n"));
}

#[test]
fn week_number_matches_partition_order() {
    let weeks = partition_year_into_month_weeks(2024);
    let week_numbers: Vec<usize> = weeks.iter().map(|w| w.week_number).collect();
    let is_monotonic = week_numbers.windows(2).all(|pair| pair[0] <= pair[1]);
    insta::assert_snapshot!(format!(
        "monotonic={} first={} last={}",
        is_monotonic,
        week_numbers.first().unwrap(),
        week_numbers.last().unwrap()
    ));
    assert!(is_monotonic);
}
