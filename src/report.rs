use std::collections::HashSet;

use anyhow::{Context, Result};
use chrono::NaiveDate;
use polars::prelude::*;

use crate::ynab::{BudgetSummary, Category, CategoryGroup, Transaction};

// --- Newtypes for DataFrames ---

#[derive(Clone)]
pub struct CategoryFrame(pub LazyFrame);

#[derive(Clone)]
pub struct TransactionFrame(pub LazyFrame);

// --- Pure functions ---

pub fn get_budget_id(budgets: &[BudgetSummary], budget_name: &str) -> Option<String> {
    budgets
        .iter()
        .find(|b| b.name == budget_name)
        .map(|b| b.id.clone())
}

pub fn get_missing_category_groups(
    groups: &[CategoryGroup],
    watch_list: &indexmap::IndexMap<String, String>,
) -> HashSet<String> {
    let available: HashSet<&str> = groups.iter().map(|g| g.name.as_str()).collect();
    watch_list
        .keys()
        .filter(|name| !available.contains(name.as_str()))
        .cloned()
        .collect()
}

pub fn get_categories_to_watch(
    groups: &[CategoryGroup],
    watch_list: &indexmap::IndexMap<String, String>,
) -> Vec<Category> {
    let watched_names: HashSet<&str> = watch_list.keys().map(String::as_str).collect();
    groups
        .iter()
        .filter(|g| watched_names.contains(g.name.as_str()))
        .flat_map(|g| g.categories.iter())
        .filter(|c| !c.hidden)
        .cloned()
        .collect()
}

fn date_to_polars_days(date: NaiveDate) -> i32 {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("valid epoch");
    (date - epoch).num_days() as i32
}

struct TransactionRow {
    date: NaiveDate,
    amount: f64,
    payee_name: Option<String>,
    category_name: String,
}

fn expand_transaction(txn: &Transaction) -> Vec<TransactionRow> {
    if !txn.subtransactions.is_empty() {
        txn.subtransactions
            .iter()
            .filter_map(|sub| {
                sub.category_name.as_ref().map(|cat_name| TransactionRow {
                    date: txn.date,
                    amount: sub.amount as f64 / 1000.0,
                    payee_name: sub
                        .payee_name
                        .clone()
                        .or_else(|| txn.payee_name.clone()),
                    category_name: cat_name.clone(),
                })
            })
            .collect()
    } else if let Some(cat_name) = &txn.category_name {
        vec![TransactionRow {
            date: txn.date,
            amount: txn.amount as f64 / 1000.0,
            payee_name: txn.payee_name.clone(),
            category_name: cat_name.clone(),
        }]
    } else {
        vec![]
    }
}

pub fn transactions_to_polars(transactions: &[Transaction]) -> Result<TransactionFrame> {
    let rows: Vec<TransactionRow> = transactions
        .iter()
        .flat_map(expand_transaction)
        .collect();

    let dates: Vec<i32> = rows.iter().map(|r| date_to_polars_days(r.date)).collect();
    let amounts: Vec<f64> = rows.iter().map(|r| r.amount).collect();
    let payees: Vec<Option<&str>> = rows
        .iter()
        .map(|r| r.payee_name.as_deref())
        .collect();
    let categories: Vec<&str> = rows.iter().map(|r| r.category_name.as_str()).collect();

    let date_series = Column::new("date".into(), &dates)
        .cast(&DataType::Date)
        .context("casting date column")?;
    let df = DataFrame::new(vec![
        date_series,
        Column::new("amount".into(), &amounts),
        Column::new("payee_name".into(), &payees),
        Column::new("category_name".into(), &categories),
    ])
    .context("building transactions DataFrame")?;

    Ok(TransactionFrame(df.lazy()))
}

pub fn categories_to_polars(categories: &[Category]) -> Result<CategoryFrame> {
    let names: Vec<&str> = categories.iter().map(|c| c.name.as_str()).collect();
    let group_names: Vec<&str> = categories
        .iter()
        .map(|c| {
            c.category_group_name
                .as_deref()
                .unwrap_or("Uncategorized")
        })
        .collect();
    let budgeted: Vec<f64> = categories
        .iter()
        .map(|c| c.budgeted as f64 / 1000.0)
        .collect();
    let balance: Vec<f64> = categories
        .iter()
        .map(|c| c.balance as f64 / 1000.0)
        .collect();
    let goal_cadence: Vec<&str> = categories
        .iter()
        .map(|c| {
            if c.goal_target.is_some() && c.goal_cadence == Some(1) {
                "monthly"
            } else {
                "annual"
            }
        })
        .collect();

    let df = DataFrame::new(vec![
        Column::new("category_name".into(), &names),
        Column::new("category_group_name".into(), &group_names),
        Column::new("budgeted".into(), &budgeted),
        Column::new("balance".into(), &balance),
        Column::new("goal_cadence".into(), &goal_cadence),
    ])
    .context("building categories DataFrame")?;

    Ok(CategoryFrame(df.lazy()))
}

pub fn relevant_transactions(
    tf: TransactionFrame,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> TransactionFrame {
    let start = date_to_polars_days(start_date);
    let end = date_to_polars_days(end_date);
    TransactionFrame(
        tf.0.filter(
            col("date")
                .cast(DataType::Int32)
                .gt_eq(lit(start))
                .and(col("date").cast(DataType::Int32).lt_eq(lit(end))),
        ),
    )
}

pub fn build_report_table(
    categories: CategoryFrame,
    transactions: TransactionFrame,
    category_names: &HashSet<String>,
) -> Result<LazyFrame> {
    let names_vec: Vec<&str> = category_names.iter().map(String::as_str).collect();
    let names_series = Series::new("_cat_filter".into(), &names_vec);

    let total_spent = transactions
        .0
        .filter(col("category_name").is_in(lit(names_series)))
        .group_by([col("category_name")])
        .agg([col("amount").sum().alias("spent")]);

    let report = categories
        .0
        .join(
            total_spent,
            [col("category_name")],
            [col("category_name")],
            JoinArgs::new(JoinType::Left),
        )
        .with_columns([col("spent").fill_null(lit(0.0))])
        .select([
            col("category_group_name"),
            col("category_name"),
            col("budgeted"),
            col("spent"),
            col("balance"),
            col("goal_cadence"),
        ])
        .sort(
            ["category_group_name", "category_name"],
            SortMultipleOptions::default(),
        );

    Ok(report)
}

pub fn build_category_group_totals_table(report_table: LazyFrame) -> Result<LazyFrame> {
    let group_totals = report_table
        .clone()
        .group_by([col("category_group_name")])
        .agg([
            col("budgeted").sum().alias("budgeted"),
            col("spent").sum().alias("spent"),
            col("balance").sum().alias("balance"),
        ])
        .select([
            col("category_group_name"),
            col("budgeted"),
            col("spent"),
            col("balance"),
        ])
        .sort(
            ["category_group_name"],
            SortMultipleOptions::default(),
        );

    let overall_total = report_table
        .select([
            lit("Total").alias("category_group_name"),
            col("budgeted").sum().alias("budgeted"),
            col("spent").sum().alias("spent"),
            col("balance").sum().alias("balance"),
        ]);

    let result = concat(
        [group_totals, overall_total],
        UnionArgs::default(),
    )
    .context("concatenating group totals with overall total")?;

    Ok(result)
}
