use chrono::{Datelike, Duration, NaiveDate};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthWeek {
    pub month: u32,
    pub week_start: NaiveDate,
    pub week_end: NaiveDate,
    pub week_number: usize,
}

impl MonthWeek {
    pub fn dates(&self) -> Vec<NaiveDate> {
        let days = (self.week_end - self.week_start).num_days();
        (0..=days)
            .map(|offset| self.week_start + Duration::days(offset))
            .collect()
    }
}

fn previous_sunday(day: NaiveDate) -> NaiveDate {
    let days_since_sunday = (day.weekday().num_days_from_sunday()) as i64;
    day - Duration::days(days_since_sunday)
}

fn week_days(week_start: NaiveDate) -> Vec<NaiveDate> {
    (0..7)
        .map(|offset| week_start + Duration::days(offset))
        .collect()
}

fn split_by_month(days: &[NaiveDate]) -> Vec<u32> {
    let mut months: Vec<u32> = days.iter().map(|d| d.month()).collect();
    months.sort_unstable();
    months.dedup();
    months
}

fn make_month_week(
    year: i32,
    month: u32,
    week_start: NaiveDate,
    week_end: NaiveDate,
    week_number: usize,
) -> MonthWeek {
    let month_first = NaiveDate::from_ymd_opt(year, month, 1).expect("valid month start");
    let days_in_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .expect("valid next month")
    .pred_opt()
    .expect("valid last day")
    .day();
    let month_last = NaiveDate::from_ymd_opt(year, month, days_in_month).expect("valid month end");

    MonthWeek {
        month,
        week_start: week_start.max(month_first),
        week_end: week_end.min(month_last),
        week_number,
    }
}

pub fn partition_year_into_month_weeks(year: i32) -> Vec<MonthWeek> {
    let first_day = NaiveDate::from_ymd_opt(year, 1, 1).expect("valid year start");
    let last_day = NaiveDate::from_ymd_opt(year, 12, 31).expect("valid year end");
    let anchor_week_start = previous_sunday(first_day);
    let last_week_end = previous_sunday(last_day) + Duration::days(6);
    let num_weeks = ((last_week_end - anchor_week_start).num_days() / 7) + 1;

    let mut result = Vec::new();
    for week_offset in 0..num_weeks {
        let week_number = (week_offset + 1) as usize;
        let week_start = anchor_week_start + Duration::days(7 * week_offset);
        let week_end = week_start + Duration::days(6);
        let in_year_days: Vec<NaiveDate> = week_days(week_start)
            .into_iter()
            .filter(|d| d.year() == year)
            .collect();
        for month in split_by_month(&in_year_days) {
            result.push(make_month_week(
                year,
                month,
                week_start,
                week_end,
                week_number,
            ));
        }
    }
    result
}

pub fn month_weeks(year: i32, month: u32) -> Vec<MonthWeek> {
    partition_year_into_month_weeks(year)
        .into_iter()
        .filter(|w| w.month == month)
        .collect()
}

pub fn month_week_for_date(day: NaiveDate) -> anyhow::Result<MonthWeek> {
    month_weeks(day.year(), day.month())
        .into_iter()
        .find(|w| w.week_start <= day && day <= w.week_end)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Date {} not found in month weeks for {:04}-{:02}",
                day,
                day.year(),
                day.month()
            )
        })
}
