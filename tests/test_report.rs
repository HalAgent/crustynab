use std::collections::HashSet;

use chrono::NaiveDate;
use crustynab::report;
use crustynab::ynab::{BudgetSummary, Category, CategoryGroup, SubTransaction, Transaction};

fn make_budget_summaries() -> Vec<BudgetSummary> {
    vec![
        BudgetSummary {
            id: "b1".into(),
            name: "Budget A".into(),
        },
        BudgetSummary {
            id: "b2".into(),
            name: "Budget B".into(),
        },
    ]
}

fn make_category_groups() -> Vec<CategoryGroup> {
    vec![
        CategoryGroup {
            id: "g1".into(),
            name: "Essentials".into(),
            hidden: false,
            deleted: false,
            categories: vec![
                Category {
                    id: "c1".into(),
                    name: "Groceries".into(),
                    category_group_name: Some("Essentials".into()),
                    budgeted: 50000,
                    balance: 31500,
                    goal_cadence: Some(1),
                    goal_target: Some(60000),
                    hidden: false,
                },
                Category {
                    id: "c2".into(),
                    name: "Rent".into(),
                    category_group_name: Some("Essentials".into()),
                    budgeted: 100000,
                    balance: 75000,
                    goal_cadence: Some(12),
                    goal_target: Some(120000),
                    hidden: false,
                },
            ],
        },
        CategoryGroup {
            id: "g2".into(),
            name: "Fun".into(),
            hidden: false,
            deleted: false,
            categories: vec![
                Category {
                    id: "c3".into(),
                    name: "Books".into(),
                    category_group_name: Some("Fun".into()),
                    budgeted: 10000,
                    balance: 6000,
                    goal_cadence: Some(1),
                    goal_target: None,
                    hidden: false,
                },
                Category {
                    id: "c4".into(),
                    name: "Games".into(),
                    category_group_name: Some("Fun".into()),
                    budgeted: 20000,
                    balance: 17000,
                    goal_cadence: Some(1),
                    goal_target: None,
                    hidden: false,
                },
            ],
        },
    ]
}

fn dataframe_snapshot(df: &polars::prelude::DataFrame) -> String {
    let columns = df.get_column_names_str().join(", ");

    let rows = (0..df.height())
        .map(|row_idx| {
            let values = df
                .get_columns()
                .iter()
                .map(|col| match col.get(row_idx) {
                    Ok(value) => value.to_string(),
                    Err(err) => format!("<err:{err}>"),
                })
                .collect::<Vec<_>>()
                .join(", ");

            format!("{row_idx}: [{values}]")
        })
        .collect::<Vec<_>>()
        .join("\n");

    if rows.is_empty() {
        format!("shape: {:?}\ncolumns: [{columns}]\n", df.shape())
    } else {
        format!("shape: {:?}\ncolumns: [{columns}]\n{rows}\n", df.shape())
    }
}

fn make_transactions() -> Vec<Transaction> {
    vec![
        Transaction {
            id: "t1".into(),
            date: NaiveDate::from_ymd_opt(2024, 3, 12).unwrap(),
            amount: -12500,
            payee_name: Some("Market".into()),
            category_name: Some("Groceries".into()),
            subtransactions: vec![],
        },
        Transaction {
            id: "t4".into(),
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
            id: "t3".into(),
            date: NaiveDate::from_ymd_opt(2024, 3, 14).unwrap(),
            amount: -25000,
            payee_name: Some("Landlord".into()),
            category_name: Some("Rent".into()),
            subtransactions: vec![],
        },
        Transaction {
            id: "t2".into(),
            date: NaiveDate::from_ymd_opt(2024, 3, 15).unwrap(),
            amount: -3000,
            payee_name: Some("Arcade".into()),
            category_name: Some("Games".into()),
            subtransactions: vec![],
        },
    ]
}

#[test]
fn get_budget_id_finds_match() {
    let summaries = make_budget_summaries();
    let result = report::get_budget_id(&summaries, "Budget A");
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn get_budget_id_missing_returns_none() {
    let summaries = make_budget_summaries();
    let result = report::get_budget_id(&summaries, "Nonexistent");
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn get_missing_category_groups_detects_missing() {
    let groups = make_category_groups();
    let mut watch_list = indexmap::IndexMap::new();
    watch_list.insert("Essentials".into(), "#fff".into());
    watch_list.insert("Nonexistent".into(), "#000".into());
    let missing = report::get_missing_category_groups(&groups, &watch_list);
    let mut missing_vec: Vec<&str> = missing.iter().map(String::as_str).collect();
    missing_vec.sort();
    insta::assert_snapshot!(format!("{:?}", missing_vec));
}

#[test]
fn get_categories_to_watch_filters_correctly() {
    let groups = make_category_groups();
    let mut watch_list = indexmap::IndexMap::new();
    watch_list.insert("Essentials".into(), "#fff".into());
    let cats = report::get_categories_to_watch(&groups, &watch_list);
    let mut names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
    names.sort();
    insta::assert_snapshot!(format!("{:?}", names));
}

#[test]
fn transactions_to_polars_expands_splits() {
    let transactions = make_transactions();
    let tf = report::transactions_to_polars(&transactions).unwrap();
    let df = tf.0.collect().unwrap();
    insta::assert_snapshot!(dataframe_snapshot(&df));
}

#[test]
fn categories_to_polars_converts_milliunits() {
    let groups = make_category_groups();
    let cats: Vec<Category> = groups.into_iter().flat_map(|g| g.categories).collect();
    let cf = report::categories_to_polars(&cats).unwrap();
    let df = cf.0.collect().unwrap();
    insta::assert_snapshot!(dataframe_snapshot(&df));
}

#[test]
fn relevant_transactions_filters_date_range() {
    let transactions = make_transactions();
    let tf = report::transactions_to_polars(&transactions).unwrap();
    let start = NaiveDate::from_ymd_opt(2024, 3, 12).unwrap();
    let end = NaiveDate::from_ymd_opt(2024, 3, 14).unwrap();
    let filtered = report::relevant_transactions(tf, start, end);
    let df = filtered.0.collect().unwrap();
    insta::assert_snapshot!(dataframe_snapshot(&df));
}

#[test]
fn build_report_table_sums_spent() {
    let groups = make_category_groups();
    let all_cats: Vec<Category> = groups.into_iter().flat_map(|g| g.categories).collect();
    let cf = report::categories_to_polars(&all_cats).unwrap();

    let transactions = make_transactions();
    let tf = report::transactions_to_polars(&transactions).unwrap();
    let start = NaiveDate::from_ymd_opt(2024, 3, 10).unwrap();
    let end = NaiveDate::from_ymd_opt(2024, 3, 16).unwrap();
    let tf = report::relevant_transactions(tf, start, end);

    let cat_names: HashSet<String> = all_cats.iter().map(|c| c.name.clone()).collect();
    let report = report::build_report_table(cf, tf, &cat_names).unwrap();
    let df = report.collect().unwrap();
    insta::assert_snapshot!(dataframe_snapshot(&df));
}

#[test]
fn category_group_totals_match_rows() {
    let groups = make_category_groups();
    let all_cats: Vec<Category> = groups.into_iter().flat_map(|g| g.categories).collect();
    let cf = report::categories_to_polars(&all_cats).unwrap();

    let transactions = make_transactions();
    let tf = report::transactions_to_polars(&transactions).unwrap();
    let start = NaiveDate::from_ymd_opt(2024, 3, 10).unwrap();
    let end = NaiveDate::from_ymd_opt(2024, 3, 16).unwrap();
    let tf = report::relevant_transactions(tf, start, end);

    let cat_names: HashSet<String> = all_cats.iter().map(|c| c.name.clone()).collect();
    let report = report::build_report_table(cf, tf, &cat_names).unwrap();
    let totals = report::build_category_group_totals_table(report).unwrap();
    let df = totals.collect().unwrap();
    insta::assert_snapshot!(dataframe_snapshot(&df));
}

#[test]
fn category_group_totals_include_balance_without_spending() {
    let categories = vec![
        Category {
            id: "c1".into(),
            name: "Groceries".into(),
            category_group_name: Some("Essentials".into()),
            budgeted: 50000,
            balance: 30000,
            goal_cadence: Some(1),
            goal_target: Some(60000),
            hidden: false,
        },
        Category {
            id: "c2".into(),
            name: "Savings".into(),
            category_group_name: Some("Essentials".into()),
            budgeted: 20000,
            balance: 90000,
            goal_cadence: Some(1),
            goal_target: Some(60000),
            hidden: false,
        },
    ];
    let cf = report::categories_to_polars(&categories).unwrap();

    let transactions = vec![Transaction {
        id: "t1".into(),
        date: NaiveDate::from_ymd_opt(2024, 3, 12).unwrap(),
        amount: -12500,
        payee_name: Some("Market".into()),
        category_name: Some("Groceries".into()),
        subtransactions: vec![],
    }];
    let tf = report::transactions_to_polars(&transactions).unwrap();
    let start = NaiveDate::from_ymd_opt(2024, 3, 10).unwrap();
    let end = NaiveDate::from_ymd_opt(2024, 3, 16).unwrap();
    let tf = report::relevant_transactions(tf, start, end);

    let cat_names: HashSet<String> = categories.iter().map(|c| c.name.clone()).collect();
    let report = report::build_report_table(cf, tf, &cat_names).unwrap();
    let totals = report::build_category_group_totals_table(report).unwrap();
    let df = totals.collect().unwrap();
    insta::assert_snapshot!(dataframe_snapshot(&df));
}

#[test]
fn split_transactions_are_counted_per_category() {
    let categories = vec![
        Category {
            id: "c1".into(),
            name: "Groceries".into(),
            category_group_name: Some("Essentials".into()),
            budgeted: 50000,
            balance: 30000,
            goal_cadence: Some(1),
            goal_target: Some(60000),
            hidden: false,
        },
        Category {
            id: "c2".into(),
            name: "Savings".into(),
            category_group_name: Some("Essentials".into()),
            budgeted: 20000,
            balance: 90000,
            goal_cadence: Some(1),
            goal_target: Some(60000),
            hidden: false,
        },
    ];
    let cf = report::categories_to_polars(&categories).unwrap();

    let transactions = vec![Transaction {
        id: "t2".into(),
        date: NaiveDate::from_ymd_opt(2024, 3, 11).unwrap(),
        amount: -20000,
        payee_name: Some("Market".into()),
        category_name: Some("Split".into()),
        subtransactions: vec![
            SubTransaction {
                amount: -12500,
                payee_name: None,
                category_name: Some("Groceries".into()),
            },
            SubTransaction {
                amount: -7500,
                payee_name: None,
                category_name: Some("Savings".into()),
            },
        ],
    }];
    let tf = report::transactions_to_polars(&transactions).unwrap();
    let start = NaiveDate::from_ymd_opt(2024, 3, 10).unwrap();
    let end = NaiveDate::from_ymd_opt(2024, 3, 16).unwrap();
    let tf = report::relevant_transactions(tf, start, end);

    let cat_names: HashSet<String> = categories.iter().map(|c| c.name.clone()).collect();
    let report = report::build_report_table(cf, tf, &cat_names).unwrap();
    let df = report.collect().unwrap();
    insta::assert_snapshot!(dataframe_snapshot(&df));
}

#[test]
fn transactions_with_no_category_are_filtered() {
    let transactions = vec![
        Transaction {
            id: "t1".into(),
            date: NaiveDate::from_ymd_opt(2024, 3, 12).unwrap(),
            amount: -5000,
            payee_name: Some("Shop".into()),
            category_name: None,
            subtransactions: vec![],
        },
        Transaction {
            id: "t2".into(),
            date: NaiveDate::from_ymd_opt(2024, 3, 12).unwrap(),
            amount: -3000,
            payee_name: Some("Store".into()),
            category_name: Some("Groceries".into()),
            subtransactions: vec![],
        },
    ];
    let tf = report::transactions_to_polars(&transactions).unwrap();
    let df = tf.0.collect().unwrap();
    insta::assert_snapshot!(dataframe_snapshot(&df));
}
