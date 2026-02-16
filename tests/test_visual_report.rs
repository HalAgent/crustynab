use crustynab::visual_report::{CURRENCY, build_visual_report_html, darken_hex, format_currency};
use indexmap::IndexMap;
use polars::prelude::*;

#[test]
fn format_currency_positive() {
    insta::assert_snapshot!(format_currency(18.5, true));
}

#[test]
fn format_currency_negative() {
    insta::assert_snapshot!(format_currency(-25.0, true));
}

#[test]
fn format_currency_zero_show() {
    insta::assert_snapshot!(format_currency(0.0, true));
}

#[test]
fn format_currency_zero_hide() {
    insta::assert_snapshot!(format_currency(0.0, false));
}

#[test]
fn format_currency_large_with_commas() {
    insta::assert_snapshot!(format_currency(1234567.89, true));
}

#[test]
fn darken_hex_standard() {
    insta::assert_snapshot!(darken_hex("#dfe7f5", 0.85));
}

#[test]
fn darken_hex_aggressive() {
    insta::assert_snapshot!(darken_hex("#dfe7f5", 0.7));
}

#[test]
fn darken_hex_invalid_passthrough() {
    insta::assert_snapshot!(darken_hex("not-a-color", 0.85));
}

#[test]
fn darken_hex_short_passthrough() {
    insta::assert_snapshot!(darken_hex("#fff", 0.85));
}

fn make_report_lazyframe(rows: Vec<(&str, &str, f64, f64, f64, &str)>) -> LazyFrame {
    let cat_names: Vec<&str> = rows.iter().map(|r| r.0).collect();
    let group_names: Vec<&str> = rows.iter().map(|r| r.1).collect();
    let budgeted: Vec<f64> = rows.iter().map(|r| r.2).collect();
    let spent: Vec<f64> = rows.iter().map(|r| r.3).collect();
    let balance: Vec<f64> = rows.iter().map(|r| r.4).collect();
    let cadence: Vec<&str> = rows.iter().map(|r| r.5).collect();

    DataFrame::new(vec![
        Column::new("category_name".into(), &cat_names),
        Column::new("category_group_name".into(), &group_names),
        Column::new("budgeted".into(), &budgeted),
        Column::new("spent".into(), &spent),
        Column::new("balance".into(), &balance),
        Column::new("goal_cadence".into(), &cadence),
    ])
    .unwrap()
    .lazy()
}

#[test]
fn visual_report_basic() {
    let report = make_report_lazyframe(vec![
        ("Groceries", "Essentials", 50.0, -18.5, 31.5, "monthly"),
        ("Rent", "Essentials", 100.0, -25.0, 75.0, "annual"),
        ("Books", "Fun", 10.0, -4.0, 6.0, "annual"),
        ("Games", "Fun", 20.0, -3.0, 17.0, "annual"),
    ]);

    let mut group_colors = IndexMap::new();
    group_colors.insert("Essentials".to_string(), "#dfe7f5".to_string());
    group_colors.insert("Fun".to_string(), "#f4dccb".to_string());

    let html = build_visual_report_html(
        report,
        &group_colors,
        "Week 11 (Mar 10 - Mar 16)",
        2024,
        true,
    )
    .unwrap();

    insta::assert_snapshot!(html);
}

#[test]
fn visual_report_totals_include_hidden_balance() {
    let report = make_report_lazyframe(vec![
        ("Groceries", "Essentials", 50.0, -10.0, 30.0, "monthly"),
        ("Savings", "Essentials", 20.0, 0.0, 90.0, "monthly"),
    ]);

    let mut group_colors = IndexMap::new();
    group_colors.insert("Essentials".to_string(), "#dfe7f5".to_string());

    let html = build_visual_report_html(report, &group_colors, "Week 1", 2024, false).unwrap();

    assert!(!html.contains("Savings"));
    assert!(html.contains("Total Essentials"));
    assert!(html.contains(&format!("{CURRENCY}840.00")));
    assert!(html.contains(&format!("{CURRENCY}70.00")));
    assert!(html.contains(&format!("{CURRENCY}10.00")));
    insta::assert_snapshot!(html);
}

#[test]
fn visual_report_hides_remaining_when_no_spend() {
    let report = make_report_lazyframe(vec![(
        "Zero Spend",
        "Essentials",
        50.0,
        0.0,
        50.0,
        "monthly",
    )]);

    let mut group_colors = IndexMap::new();
    group_colors.insert("Essentials".to_string(), "#dfe7f5".to_string());

    let html = build_visual_report_html(report, &group_colors, "Week 1", 2024, true).unwrap();

    assert!(html.contains("Zero Spend"));
    insta::assert_snapshot!(html);
}
