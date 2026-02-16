use std::collections::HashSet;

use anyhow::Result;
use chrono::NaiveDate;
use indexmap::IndexMap;
use polars::prelude::*;

use crustynab::calendar_weeks::month_week_for_date;
use crustynab::config::{Config, OutputFormat, SimpleOutputFormat};
use crustynab::report;
use crustynab::visual_report::build_visual_report_html;
use crustynab::ynab::{Category, SubTransaction, Transaction};

fn make_categories() -> Vec<Category> {
    vec![
        Category {
            id: "cat-groceries".into(),
            name: "Groceries".into(),
            category_group_name: Some("Essentials".into()),
            budgeted: 50000,
            balance: 31500,
            goal_cadence: Some(1),
            goal_target: Some(60000),
            hidden: false,
        },
        Category {
            id: "cat-rent".into(),
            name: "Rent".into(),
            category_group_name: Some("Essentials".into()),
            budgeted: 100000,
            balance: 75000,
            goal_cadence: Some(12),
            goal_target: Some(120000),
            hidden: false,
        },
        Category {
            id: "cat-books".into(),
            name: "Books".into(),
            category_group_name: Some("Fun".into()),
            budgeted: 10000,
            balance: 6000,
            goal_cadence: Some(1),
            goal_target: None,
            hidden: false,
        },
        Category {
            id: "cat-games".into(),
            name: "Games".into(),
            category_group_name: Some("Fun".into()),
            budgeted: 20000,
            balance: 17000,
            goal_cadence: Some(1),
            goal_target: None,
            hidden: false,
        },
    ]
}

fn make_transactions() -> Vec<Transaction> {
    vec![
        Transaction {
            id: "txn-1".into(),
            date: NaiveDate::from_ymd_opt(2024, 3, 12).unwrap(),
            amount: -12500,
            payee_name: Some("Market".into()),
            category_name: Some("Groceries".into()),
            subtransactions: vec![],
        },
        Transaction {
            id: "txn-4".into(),
            date: NaiveDate::from_ymd_opt(2024, 3, 13).unwrap(),
            amount: -10000,
            payee_name: Some("Market".into()),
            category_name: Some("Split".into()),
            subtransactions: vec![
                SubTransaction {
                    amount: -6000,
                    payee_name: None,
                    category_name: Some("Groceries".into()),
                },
                SubTransaction {
                    amount: -4000,
                    payee_name: None,
                    category_name: Some("Books".into()),
                },
            ],
        },
        Transaction {
            id: "txn-3".into(),
            date: NaiveDate::from_ymd_opt(2024, 3, 14).unwrap(),
            amount: -25000,
            payee_name: Some("Landlord".into()),
            category_name: Some("Rent".into()),
            subtransactions: vec![],
        },
        Transaction {
            id: "txn-2".into(),
            date: NaiveDate::from_ymd_opt(2024, 3, 15).unwrap(),
            amount: -3000,
            payee_name: Some("Arcade".into()),
            category_name: Some("Games".into()),
            subtransactions: vec![],
        },
    ]
}

fn make_config(show_all_rows: bool) -> Config {
    let mut watch_list = IndexMap::new();
    watch_list.insert("Essentials".to_string(), "#dfe7f5".to_string());
    watch_list.insert("Fun".to_string(), "#f4dccb".to_string());

    Config {
        budget_name: "Test Budget".to_string(),
        personal_access_token: "token".to_string(),
        category_group_watch_list: watch_list,
        resolution_date: Some(NaiveDate::from_ymd_opt(2024, 3, 13).unwrap()),
        show_all_rows,
        output_format: OutputFormat::Simple(SimpleOutputFormat::PolarsPrint),
    }
}

fn run_report(
    cfg: &Config,
) -> Result<(String, LazyFrame, LazyFrame)> {
    let categories = make_categories();
    let transactions = make_transactions();

    let resolution_date = cfg.resolution_date.expect("test has resolution_date");
    let report_week = month_week_for_date(resolution_date)?;
    let report_start = report_week.week_start;
    let report_end = report_week.week_end;

    let categories_budgeted = report::categories_to_polars(&categories)?;
    let transactions_frame = report::transactions_to_polars(&transactions)?;
    let transactions_frame =
        report::relevant_transactions(transactions_frame, report_start, report_end);

    let cat_names: HashSet<String> = categories.iter().map(|c| c.name.clone()).collect();
    let report_table = report::build_report_table(categories_budgeted, transactions_frame, &cat_names)?;

    let report_table_full = report_table.clone();
    let report_table_display = if cfg.show_all_rows {
        report_table
    } else {
        report_table.filter(col("spent").neq(lit(0.0)))
    };

    let category_group_totals = report::build_category_group_totals_table(report_table_full.clone())?;

    let week_year = report_week.week_start.year();
    let week_number = report_week.week_number;
    let start_label = report_start.format("%A %Y-%m-%d");
    let end_label = report_end.format("%A %Y-%m-%d");
    let header = format!(
        "Week {week_number} of {week_year}, starting on {start_label} and ending on {end_label}"
    );

    use chrono::Datelike;
    Ok((header, report_table_display, category_group_totals))
}

fn write_csv_string(df: &mut DataFrame) -> String {
    let mut buf = Vec::new();
    CsvWriter::new(&mut buf).finish(df).unwrap();
    String::from_utf8(buf).unwrap()
}

fn format_short_date(date: NaiveDate) -> String {
    let formatted = date.format("%b %d").to_string();
    if let Some(space_pos) = formatted.rfind(' ') {
        let (prefix, day_part) = formatted.split_at(space_pos + 1);
        if day_part.starts_with('0') {
            return format!("{}{}", prefix, &day_part[1..]);
        }
    }
    formatted
}

#[test]
fn golden_polars_print() {
    let cfg = make_config(true);
    let (header, report_display, totals) = run_report(&cfg).unwrap();
    unsafe { std::env::set_var("POLARS_FMT_MAX_ROWS", "-1") };
    let df = report_display.collect().unwrap();
    let totals_df = totals.collect().unwrap();
    let output = format!("{header}\n{df}\nCategory group totals\n{totals_df}\n");
    insta::assert_snapshot!(output);
}

#[test]
fn golden_csv_print() {
    let cfg = make_config(true);
    let (header, report_display, totals) = run_report(&cfg).unwrap();
    let mut df = report_display.collect().unwrap();
    let mut totals_df = totals.collect().unwrap();
    let csv = write_csv_string(&mut df);
    let totals_csv = write_csv_string(&mut totals_df);
    let output = format!("{header}\n{csv}category_group_totals\n{totals_csv}");
    insta::assert_snapshot!(output);
}

#[test]
fn golden_csv_output_files() {
    let cfg = make_config(true);
    let (_, report_display, totals) = run_report(&cfg).unwrap();
    let mut df = report_display.collect().unwrap();
    let mut totals_df = totals.collect().unwrap();
    let csv = write_csv_string(&mut df);
    let totals_csv = write_csv_string(&mut totals_df);
    insta::assert_snapshot!("golden_csv_report", csv);
    insta::assert_snapshot!("golden_csv_totals", totals_csv);
}

#[test]
fn golden_visual_output() {
    let cfg = make_config(true);
    let resolution_date = cfg.resolution_date.unwrap();
    let report_week = month_week_for_date(resolution_date).unwrap();

    let categories = make_categories();
    let transactions = make_transactions();
    let categories_budgeted = report::categories_to_polars(&categories).unwrap();
    let transactions_frame = report::transactions_to_polars(&transactions).unwrap();
    let transactions_frame = report::relevant_transactions(
        transactions_frame,
        report_week.week_start,
        report_week.week_end,
    );
    let cat_names: HashSet<String> = categories.iter().map(|c| c.name.clone()).collect();
    let report_table =
        report::build_report_table(categories_budgeted, transactions_frame, &cat_names).unwrap();

    use chrono::Datelike;
    let week_number = report_week.week_number;
    let week_short_start = format_short_date(report_week.week_start);
    let week_short_end = format_short_date(report_week.week_end);
    let week_label = format!("Week {week_number} ({week_short_start} - {week_short_end})");

    let html = build_visual_report_html(
        report_table,
        &cfg.category_group_watch_list,
        &week_label,
        report_week.week_start.year(),
        true,
    )
    .unwrap();
    insta::assert_snapshot!(html);
}
