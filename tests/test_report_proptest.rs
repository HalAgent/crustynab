use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{Duration, NaiveDate};
use crustynab::report::{self, CategoryFrame, TransactionFrame};
use crustynab::ynab::{BudgetSummary, CategoryGroup, SubTransaction, Transaction};
use polars::prelude::*;
use proptest::prelude::*;
use proptest::string::string_regex;

#[derive(Clone, Debug)]
struct CategoryRow {
    category_name: String,
    category_group_name: String,
    budgeted: f64,
    balance: f64,
    goal_cadence: String,
}

#[derive(Clone, Debug)]
struct TxRow {
    date: NaiveDate,
    amount_milli: i64,
    payee_name: Option<String>,
    category_name: String,
}

fn short_text_strategy() -> impl Strategy<Value = String> {
    string_regex("[a-z]{1,10}").expect("regex")
}

fn short_id_strategy() -> impl Strategy<Value = String> {
    string_regex("[a-z]{1,12}").expect("regex")
}

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

fn date_strategy(min: NaiveDate, max: NaiveDate) -> impl Strategy<Value = NaiveDate> {
    let span = (max - min).num_days();
    (0_i64..=span).prop_map(move |offset| min + Duration::days(offset))
}

fn unique(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn category_frame(rows: &[CategoryRow]) -> CategoryFrame {
    let category_names: Vec<&str> = rows.iter().map(|row| row.category_name.as_str()).collect();
    let group_names: Vec<&str> = rows
        .iter()
        .map(|row| row.category_group_name.as_str())
        .collect();
    let budgeted: Vec<f64> = rows.iter().map(|row| row.budgeted).collect();
    let balance: Vec<f64> = rows.iter().map(|row| row.balance).collect();
    let goal_cadence: Vec<&str> = rows.iter().map(|row| row.goal_cadence.as_str()).collect();

    let df = DataFrame::new(vec![
        Column::new("category_name".into(), &category_names),
        Column::new("category_group_name".into(), &group_names),
        Column::new("budgeted".into(), &budgeted),
        Column::new("balance".into(), &balance),
        Column::new("goal_cadence".into(), &goal_cadence),
    ])
    .expect("category frame");

    CategoryFrame(df.lazy())
}

fn date_to_polars_days(day: NaiveDate) -> i32 {
    let epoch = date(1970, 1, 1);
    (day - epoch).num_days() as i32
}

fn polars_days_to_date(days: i32) -> NaiveDate {
    date(1970, 1, 1) + Duration::days(days as i64)
}

fn transaction_frame(rows: &[TxRow]) -> TransactionFrame {
    let dates_days: Vec<i32> = rows
        .iter()
        .map(|row| date_to_polars_days(row.date))
        .collect();
    let amounts: Vec<f64> = rows
        .iter()
        .map(|row| row.amount_milli as f64 / 1000.0)
        .collect();
    let payees: Vec<Option<&str>> = rows.iter().map(|row| row.payee_name.as_deref()).collect();
    let category_names: Vec<&str> = rows.iter().map(|row| row.category_name.as_str()).collect();

    let date_col = Column::new("date".into(), &dates_days)
        .cast(&DataType::Date)
        .expect("date cast");

    let df = DataFrame::new(vec![
        date_col,
        Column::new("amount".into(), &amounts),
        Column::new("payee_name".into(), &payees),
        Column::new("category_name".into(), &category_names),
    ])
    .expect("transaction frame");

    TransactionFrame(df.lazy())
}

fn report_spent_map(df: &DataFrame) -> HashMap<String, f64> {
    let categories = df
        .column("category_name")
        .expect("category_name")
        .str()
        .expect("category_name str");
    let spent = df.column("spent").expect("spent").f64().expect("spent f64");

    let mut map = HashMap::new();
    for idx in 0..df.height() {
        let category = categories.get(idx).expect("category").to_string();
        map.insert(category, spent.get(idx).expect("spent value"));
    }
    map
}

fn transaction_multiset(df: &DataFrame) -> BTreeMap<String, usize> {
    let date_col = df
        .column("date")
        .expect("date")
        .cast(&DataType::Int32)
        .expect("date cast to i32");
    let date_days = date_col.i32().expect("date i32");
    let amounts = df
        .column("amount")
        .expect("amount")
        .f64()
        .expect("amount f64");
    let payees = df
        .column("payee_name")
        .expect("payee_name")
        .str()
        .expect("payee_name str");
    let categories = df
        .column("category_name")
        .expect("category_name")
        .str()
        .expect("category_name str");

    let mut counts = BTreeMap::new();
    for idx in 0..df.height() {
        let day = polars_days_to_date(date_days.get(idx).expect("day value"));
        let milli = (amounts.get(idx).expect("amount value") * 1000.0).round() as i64;
        let payee = payees.get(idx).unwrap_or("<none>");
        let category = categories.get(idx).expect("category value");
        let key = format!("{day}|{milli}|{payee}|{category}");
        *counts.entry(key).or_insert(0) += 1;
    }

    counts
}

fn report_totals_map(df: &DataFrame) -> HashMap<String, (f64, f64, f64)> {
    let groups = df
        .column("category_group_name")
        .expect("category_group_name")
        .str()
        .expect("category_group_name str");
    let budgeted = df
        .column("budgeted")
        .expect("budgeted")
        .f64()
        .expect("budgeted f64");
    let spent = df.column("spent").expect("spent").f64().expect("spent f64");
    let balance = df
        .column("balance")
        .expect("balance")
        .f64()
        .expect("balance f64");

    let mut map = HashMap::new();
    for idx in 0..df.height() {
        let group = groups.get(idx).expect("group value").to_string();
        map.insert(
            group,
            (
                budgeted.get(idx).expect("budgeted value"),
                spent.get(idx).expect("spent value"),
                balance.get(idx).expect("balance value"),
            ),
        );
    }
    map
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn category_rows_strategy() -> impl Strategy<Value = Vec<CategoryRow>> {
    (
        prop::collection::vec(short_text_strategy(), 1..=8),
        prop::collection::vec(short_text_strategy(), 1..=4),
    )
        .prop_flat_map(|(raw_names, raw_groups)| {
            let names = unique(raw_names);
            let groups = unique(raw_groups);
            let len = names.len();

            (
                Just(names),
                Just(groups.clone()),
                prop::collection::vec(0usize..groups.len(), len),
                prop::collection::vec(-1_000_000_i64..=1_000_000_i64, len),
                prop::collection::vec(-1_000_000_i64..=1_000_000_i64, len),
                prop::collection::vec(any::<bool>(), len),
            )
        })
        .prop_map(
            |(names, groups, group_indexes, budgeted, balance, monthly)| {
                names
                    .into_iter()
                    .enumerate()
                    .map(|(idx, category_name)| CategoryRow {
                        category_name,
                        category_group_name: groups[group_indexes[idx]].clone(),
                        budgeted: budgeted[idx] as f64 / 1000.0,
                        balance: balance[idx] as f64 / 1000.0,
                        goal_cadence: if monthly[idx] {
                            "monthly".to_string()
                        } else {
                            "annual".to_string()
                        },
                    })
                    .collect()
            },
        )
}

fn transaction_rows_for_categories(
    category_names: Vec<String>,
) -> impl Strategy<Value = Vec<TxRow>> {
    let category_strategy = prop_oneof![
        3 => prop::sample::select(category_names),
        1 => short_text_strategy().prop_map(|name| format!("other_{name}")),
    ];

    let start = date(2000, 1, 1);
    let end = date(2030, 12, 31);

    prop::collection::vec(
        (
            date_strategy(start, end),
            -1_000_000_i64..=1_000_000_i64,
            prop::option::of(short_text_strategy()),
            category_strategy,
        ),
        0..=25,
    )
    .prop_map(|rows| {
        rows.into_iter()
            .map(|(date, amount_milli, payee_name, category_name)| TxRow {
                date,
                amount_milli,
                payee_name,
                category_name,
            })
            .collect()
    })
}

fn categories_and_transactions_strategy() -> impl Strategy<Value = (Vec<CategoryRow>, Vec<TxRow>)> {
    category_rows_strategy().prop_flat_map(|categories| {
        let category_names = categories
            .iter()
            .map(|row| row.category_name.clone())
            .collect::<Vec<_>>();
        transaction_rows_for_categories(category_names)
            .prop_map(move |transactions| (categories.clone(), transactions))
    })
}

fn transaction_rows_any_strategy() -> impl Strategy<Value = Vec<TxRow>> {
    let start = date(2000, 1, 1);
    let end = date(2030, 12, 31);

    prop::collection::vec(
        (
            date_strategy(start, end),
            -1_000_000_i64..=1_000_000_i64,
            prop::option::of(short_text_strategy()),
            short_text_strategy(),
        ),
        0..=25,
    )
    .prop_map(|rows| {
        rows.into_iter()
            .map(|(date, amount_milli, payee_name, category_name)| TxRow {
                date,
                amount_milli,
                payee_name,
                category_name,
            })
            .collect()
    })
}

fn budget_summaries_strategy() -> impl Strategy<Value = Vec<BudgetSummary>> {
    prop::collection::vec(short_text_strategy(), 1..=8).prop_map(|names| {
        unique(names)
            .into_iter()
            .enumerate()
            .map(|(idx, name)| BudgetSummary {
                id: format!("budget-{idx}"),
                name,
            })
            .collect()
    })
}

fn budget_summaries_and_target_strategy() -> impl Strategy<Value = (Vec<BudgetSummary>, String)> {
    budget_summaries_strategy().prop_flat_map(|summaries| {
        let targets = summaries
            .iter()
            .map(|summary| summary.name.clone())
            .collect::<Vec<_>>();
        (Just(summaries), prop::sample::select(targets))
    })
}

fn subtransaction_strategy() -> impl Strategy<Value = SubTransaction> {
    (
        -1_000_000_i64..=1_000_000_i64,
        prop::option::of(short_text_strategy()),
        prop::option::of(short_text_strategy()),
    )
        .prop_map(|(amount, payee_name, category_name)| SubTransaction {
            amount,
            payee_name,
            category_name,
        })
}

fn transaction_strategy() -> impl Strategy<Value = Transaction> {
    (
        short_id_strategy(),
        date_strategy(date(2000, 1, 1), date(2030, 12, 31)),
        -1_000_000_i64..=1_000_000_i64,
        prop::option::of(short_text_strategy()),
        prop::collection::vec(subtransaction_strategy(), 0..=3),
        prop::option::of(short_text_strategy()),
    )
        .prop_map(
            |(id, date, amount, payee_name, subtransactions, category_name)| {
                let category_name = if subtransactions.is_empty() {
                    category_name
                } else {
                    Some("Split".to_string())
                };

                Transaction {
                    id,
                    date,
                    amount,
                    payee_name,
                    category_name,
                    subtransactions,
                }
            },
        )
}

fn transaction_details_strategy() -> impl Strategy<Value = Vec<Transaction>> {
    prop::collection::vec(transaction_strategy(), 0..=15)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn prop_get_missing_category_groups(
        group_names in prop::collection::hash_set(short_text_strategy(), 0..=8),
        watch_names in prop::collection::hash_set(short_text_strategy(), 0..=8),
    ) {
        let groups = group_names
            .iter()
            .map(|name| CategoryGroup {
                id: format!("group-{name}"),
                name: name.clone(),
                hidden: false,
                deleted: false,
                categories: vec![],
            })
            .collect::<Vec<_>>();

        let watch_list = watch_names
            .iter()
            .map(|name| (name.clone(), "#ffffff".to_string()))
            .collect();

        let missing = report::get_missing_category_groups(&groups, &watch_list);
        let expected = watch_names.difference(&group_names).cloned().collect::<HashSet<_>>();

        prop_assert_eq!(missing, expected);
    }

    #[test]
    fn prop_get_budget_id_finds_match((summaries, target) in budget_summaries_and_target_strategy()) {
        let expected = summaries
            .iter()
            .find(|summary| summary.name == target)
            .expect("target must exist")
            .id
            .clone();

        prop_assert_eq!(report::get_budget_id(&summaries, &target), Some(expected));
    }

    #[test]
    fn prop_get_budget_id_missing_returns_none(
        summaries in budget_summaries_strategy(),
        missing_name in short_text_strategy(),
    ) {
        let names = summaries.iter().map(|summary| summary.name.as_str()).collect::<HashSet<_>>();
        prop_assume!(!names.contains(missing_name.as_str()));

        prop_assert_eq!(report::get_budget_id(&summaries, &missing_name), None);
    }

    #[test]
    fn prop_build_report_table_sums_spent((categories, transactions) in categories_and_transactions_strategy()) {
        let category_names = categories
            .iter()
            .map(|row| row.category_name.clone())
            .collect::<HashSet<_>>();

        let categories_frame = category_frame(&categories);
        let transactions_frame = transaction_frame(&transactions);

        let report_df = report::build_report_table(categories_frame, transactions_frame, &category_names)
            .expect("build_report_table")
            .collect()
            .expect("collect report");

        let mut expected = HashMap::<String, f64>::new();
        for tx in transactions {
            if category_names.contains(&tx.category_name) {
                *expected.entry(tx.category_name.clone()).or_insert(0.0) += tx.amount_milli as f64 / 1000.0;
            }
        }

        let actual = report_spent_map(&report_df);
        prop_assert_eq!(actual.len(), category_names.len());

        for category in category_names {
            let actual_spent = actual.get(&category).copied().unwrap_or(0.0);
            let expected_spent = expected.get(&category).copied().unwrap_or(0.0);
            prop_assert!(close(actual_spent, expected_spent));
        }
    }

    #[test]
    fn prop_relevant_transactions_filters_range(
        rows in transaction_rows_any_strategy(),
        start in date_strategy(date(2000, 1, 1), date(2030, 12, 31)),
        end in date_strategy(date(2000, 1, 1), date(2030, 12, 31)),
    ) {
        let (start, end) = if start <= end { (start, end) } else { (end, start) };
        let frame = transaction_frame(&rows);
        let filtered_df = report::relevant_transactions(frame, start, end)
            .0
            .collect()
            .expect("collect filtered");

        let expected_rows = rows
            .iter()
            .filter(|row| start <= row.date && row.date <= end)
            .map(|row| {
                let payee = row.payee_name.clone().unwrap_or_else(|| "<none>".to_string());
                format!("{}|{}|{}|{}", row.date, row.amount_milli, payee, row.category_name)
            })
            .fold(BTreeMap::<String, usize>::new(), |mut acc, key| {
                *acc.entry(key).or_insert(0) += 1;
                acc
            });

        let actual_rows = transaction_multiset(&filtered_df);
        prop_assert_eq!(actual_rows, expected_rows);
    }

    #[test]
    fn prop_category_group_totals_match_rows((categories, transactions) in categories_and_transactions_strategy()) {
        let category_names = categories
            .iter()
            .map(|row| row.category_name.clone())
            .collect::<HashSet<_>>();

        let report_table = report::build_report_table(
            category_frame(&categories),
            transaction_frame(&transactions),
            &category_names,
        )
        .expect("build_report_table");

        let report_df = report_table.clone().collect().expect("collect report table");
        let totals_df = report::build_category_group_totals_table(report_table)
            .expect("build totals")
            .collect()
            .expect("collect totals");

        let groups = report_df
            .column("category_group_name")
            .expect("category_group_name")
            .str()
            .expect("category_group_name str");
        let budgeted = report_df.column("budgeted").expect("budgeted").f64().expect("budgeted f64");
        let spent = report_df.column("spent").expect("spent").f64().expect("spent f64");
        let balance = report_df.column("balance").expect("balance").f64().expect("balance f64");

        let mut expected = HashMap::<String, (f64, f64, f64)>::new();
        for idx in 0..report_df.height() {
            let group = groups.get(idx).expect("group value").to_string();
            let entry = expected.entry(group).or_insert((0.0, 0.0, 0.0));
            entry.0 += budgeted.get(idx).expect("budgeted value");
            entry.1 += spent.get(idx).expect("spent value");
            entry.2 += balance.get(idx).expect("balance value");
        }

        let actual = report_totals_map(&totals_df);

        for (group, (exp_budgeted, exp_spent, exp_balance)) in &expected {
            let (act_budgeted, act_spent, act_balance) = actual
                .get(group)
                .copied()
                .expect("group exists in totals");
            prop_assert!(close(act_budgeted, *exp_budgeted));
            prop_assert!(close(act_spent, *exp_spent));
            prop_assert!(close(act_balance, *exp_balance));
        }

        let (total_budgeted, total_spent, total_balance) = actual
            .get("Total")
            .copied()
            .expect("overall total exists");

        let expected_budgeted: f64 = expected.values().map(|values| values.0).sum();
        let expected_spent: f64 = expected.values().map(|values| values.1).sum();
        let expected_balance: f64 = expected.values().map(|values| values.2).sum();

        prop_assert!(close(total_budgeted, expected_budgeted));
        prop_assert!(close(total_spent, expected_spent));
        prop_assert!(close(total_balance, expected_balance));
    }

    #[test]
    fn prop_transactions_to_polars_matches_manual_rows(transactions in transaction_details_strategy()) {
        let mut expected_rows = Vec::<String>::new();

        for transaction in &transactions {
            if !transaction.subtransactions.is_empty() {
                for sub in &transaction.subtransactions {
                    if let Some(category_name) = &sub.category_name {
                        let payee = sub
                            .payee_name
                            .as_ref()
                            .or(transaction.payee_name.as_ref())
                            .map(String::as_str)
                            .unwrap_or("<none>");
                        expected_rows.push(format!(
                            "{}|{}|{}|{}",
                            transaction.date,
                            sub.amount,
                            payee,
                            category_name,
                        ));
                    }
                }
            } else if let Some(category_name) = &transaction.category_name {
                let payee = transaction.payee_name.as_deref().unwrap_or("<none>");
                expected_rows.push(format!(
                    "{}|{}|{}|{}",
                    transaction.date,
                    transaction.amount,
                    payee,
                    category_name,
                ));
            }
        }

        let df = report::transactions_to_polars(&transactions)
            .expect("transactions_to_polars")
            .0
            .collect()
            .expect("collect transaction frame");

        let actual_date_col = df
            .column("date")
            .expect("date")
            .cast(&DataType::Int32)
            .expect("cast date");
        let actual_days = actual_date_col.i32().expect("date i32");
        let actual_amounts = df.column("amount").expect("amount").f64().expect("amount f64");
        let actual_payees = df
            .column("payee_name")
            .expect("payee_name")
            .str()
            .expect("payee_name str");
        let actual_categories = df
            .column("category_name")
            .expect("category_name")
            .str()
            .expect("category_name str");

        let mut actual_rows = Vec::<String>::new();
        for idx in 0..df.height() {
            let day = polars_days_to_date(actual_days.get(idx).expect("day"));
            let amount_milli = (actual_amounts.get(idx).expect("amount") * 1000.0).round() as i64;
            let payee = actual_payees.get(idx).unwrap_or("<none>");
            let category = actual_categories.get(idx).expect("category");
            actual_rows.push(format!("{day}|{amount_milli}|{payee}|{category}"));
        }

        prop_assert_eq!(actual_rows, expected_rows);
    }
}
