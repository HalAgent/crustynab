use chrono::{Datelike, Duration, NaiveDate, Weekday};
use crustynab::calendar_weeks::{
    month_week_for_date, month_weeks, partition_year_into_month_weeks,
};
use proptest::prelude::*;

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

fn month_last_day(year: i32, month: u32) -> NaiveDate {
    if month == 12 {
        date(year + 1, 1, 1)
    } else {
        date(year, month + 1, 1)
    }
    .pred_opt()
    .expect("valid previous day")
}

fn previous_sunday(day: NaiveDate) -> NaiveDate {
    day - Duration::days(day.weekday().num_days_from_sunday() as i64)
}

fn full_date_strategy(min: NaiveDate, max: NaiveDate) -> impl Strategy<Value = NaiveDate> {
    let span = (max - min).num_days();
    (0_i64..=span).prop_map(move |offset| min + Duration::days(offset))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn prop_partition_covers_year(year in 1900_i32..=2100_i32) {
        let weeks = partition_year_into_month_weeks(year);
        let all_days: Vec<NaiveDate> = weeks.iter().flat_map(|w| w.dates()).collect();

        let mut dedup = all_days.clone();
        dedup.sort_unstable();
        dedup.dedup();

        let year_start = date(year, 1, 1);
        let year_end = date(year, 12, 31);
        let expected_count = (year_end - year_start).num_days() + 1;

        prop_assert_eq!(all_days.len(), dedup.len());
        prop_assert_eq!(dedup.len() as i64, expected_count);
        prop_assert_eq!(dedup.first().copied(), Some(year_start));
        prop_assert_eq!(dedup.last().copied(), Some(year_end));
    }

    #[test]
    fn prop_month_partition_covers_month(year in 1900_i32..=2100_i32, month in 1_u32..=12_u32) {
        let weeks = month_weeks(year, month);
        let all_days: Vec<NaiveDate> = weeks.iter().flat_map(|w| w.dates()).collect();

        let mut dedup = all_days.clone();
        dedup.sort_unstable();
        dedup.dedup();

        let first = date(year, month, 1);
        let last = month_last_day(year, month);
        let expected_count = (last - first).num_days() + 1;

        prop_assert_eq!(all_days.len(), dedup.len());
        prop_assert_eq!(dedup.len() as i64, expected_count);
        prop_assert_eq!(dedup.first().copied(), Some(first));
        prop_assert_eq!(dedup.last().copied(), Some(last));
    }

    #[test]
    fn prop_week_invariants(year in 1900_i32..=2100_i32, month in 1_u32..=12_u32) {
        let weeks = month_weeks(year, month);
        let month_first = date(year, month, 1);
        let month_last = month_last_day(year, month);

        for week in weeks {
            let start_is_sunday = week.week_start.weekday() == Weekday::Sun;
            let end_is_saturday = week.week_end.weekday() == Weekday::Sat;

            prop_assert!(start_is_sunday || week.week_start == month_first);
            prop_assert!(end_is_saturday || week.week_end == month_last);
            prop_assert!(start_is_sunday || end_is_saturday);

            let days = week.dates();
            prop_assert!(!days.is_empty());
            prop_assert!(days.iter().all(|d| d.year() == year));
            prop_assert!(days.iter().all(|d| d.month() == month));

            let span = (days[days.len() - 1] - days[0]).num_days();
            prop_assert_eq!(span, days.len() as i64 - 1);
        }
    }

    #[test]
    fn prop_month_week_for_date_contains_date(
        day in full_date_strategy(date(1900, 1, 1), date(2100, 12, 31))
    ) {
        let week = month_week_for_date(day).expect("date must resolve to month week");

        prop_assert_eq!(week.month, day.month());
        prop_assert!(week.week_start <= day && day <= week.week_end);

        let month_partition = month_weeks(day.year(), day.month());
        prop_assert!(month_partition.contains(&week));
    }

    #[test]
    fn prop_week_number_matches_partition_order(year in 1900_i32..=2100_i32) {
        let weeks = partition_year_into_month_weeks(year);
        let year_anchor = previous_sunday(date(year, 1, 1));

        for week in weeks {
            let anchor = previous_sunday(week.week_start);
            let expected = ((anchor - year_anchor).num_days() / 7) as usize + 1;
            prop_assert_eq!(week.week_number, expected);
        }
    }
}
