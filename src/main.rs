use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Datelike;
use clap::Parser;
use polars::prelude::*;

use crustynab::calendar_weeks::month_week_for_date;
use crustynab::config::{self, OutputFormat, SimpleOutputFormat};
use crustynab::report;
use crustynab::visual_report::build_visual_report_html;
use crustynab::ynab::{HttpYnabClient, YnabApi};

#[derive(Parser, Debug)]
#[clap(author = "Simon Zeng", version, about = "YNAB budget reporting tool")]
struct Args {
    /// Path to config.json
    #[arg(short, long, default_value = "config.json")]
    config: PathBuf,
}

pub fn run(api: &dyn YnabApi, cfg: &config::Config) -> Result<()> {
    let budgets = api.get_budgets()?;
    let budget_id = report::get_budget_id(&budgets, &cfg.budget_name)
        .ok_or_else(|| anyhow::anyhow!("no budget found with name {}", cfg.budget_name))?;

    let category_groups = api.get_category_groups(&budget_id)?;
    let missing =
        report::get_missing_category_groups(&category_groups, &cfg.category_group_watch_list);
    if !missing.is_empty() {
        let mut names: Vec<&str> = missing.iter().map(String::as_str).collect();
        names.sort();
        eprintln!(
            "Warning: categoryGroupWatchList includes unknown category groups: {}",
            names.join(", ")
        );
    }

    let categories_to_watch =
        report::get_categories_to_watch(&category_groups, &cfg.category_group_watch_list);

    let resolution_date = cfg
        .resolution_date
        .unwrap_or_else(|| chrono::Local::now().date_naive());
    let report_week = month_week_for_date(resolution_date)?;
    let report_start = report_week.week_start;
    let report_end = report_week.week_end;

    let month_categories: Vec<_> = categories_to_watch
        .iter()
        .map(|cat| api.get_month_category(&budget_id, report_start, &cat.id))
        .collect::<Result<Vec<_>>>()
        .context("fetching month categories")?;

    let categories_budgeted = report::categories_to_polars(&month_categories)?;

    let transactions = api.get_transactions(&budget_id, report_start)?;
    let transactions_frame = report::transactions_to_polars(&transactions)?;
    let transactions_frame =
        report::relevant_transactions(transactions_frame, report_start, report_end);

    let category_names: HashSet<String> =
        month_categories.iter().map(|c| c.name.clone()).collect();

    let report_table =
        report::build_report_table(categories_budgeted, transactions_frame, &category_names)?;

    let report_table_full = report_table.clone();
    let report_table_display = if cfg.show_all_rows {
        report_table
    } else {
        report_table.filter(col("spent").neq(lit(0.0)))
    };

    let category_group_totals =
        report::build_category_group_totals_table(report_table_full.clone())?;

    let week_year = report_week.week_start.year();
    let week_number = report_week.week_number;
    let start_label = report_start.format("%A %Y-%m-%d");
    let end_label = report_end.format("%A %Y-%m-%d");
    println!(
        "Week {week_number} of {week_year}, starting on {start_label} and ending on {end_label}"
    );

    let week_short_start = format_short_date(report_start);
    let week_short_end = format_short_date(report_end);
    let visual_week_label = format!("Week {week_number} ({week_short_start} - {week_short_end})");

    match &cfg.output_format {
        OutputFormat::Simple(SimpleOutputFormat::PolarsPrint) => {
            // SAFETY: single-threaded at this point, no concurrent env access
            unsafe { std::env::set_var("POLARS_FMT_MAX_ROWS", "-1") };
            let df = report_table_display
                .collect()
                .context("collecting report table")?;
            let totals = category_group_totals
                .collect()
                .context("collecting totals")?;
            println!("{df}");
            println!("Category group totals");
            println!("{totals}");
        }
        OutputFormat::Simple(SimpleOutputFormat::CsvPrint) => {
            let mut df = report_table_display
                .collect()
                .context("collecting report table")?;
            let mut totals = category_group_totals
                .collect()
                .context("collecting totals")?;
            let csv = write_csv_string(&mut df)?;
            let totals_csv = write_csv_string(&mut totals)?;
            print!("{csv}");
            println!("category_group_totals");
            print!("{totals_csv}");
        }
        OutputFormat::CsvFile { csv_output } => {
            let mut df = report_table_display
                .collect()
                .context("collecting report table")?;
            let mut totals = category_group_totals
                .collect()
                .context("collecting totals")?;
            let csv = write_csv_string(&mut df)?;
            let totals_csv = write_csv_string(&mut totals)?;

            let stem = csv_output
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("report");
            let ext = csv_output
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("csv");
            let totals_path =
                csv_output.with_file_name(format!("{stem}_category_group_totals.{ext}"));

            std::fs::write(csv_output, &csv)
                .with_context(|| format!("writing {csv_output:?}"))?;
            std::fs::write(&totals_path, &totals_csv)
                .with_context(|| format!("writing {totals_path:?}"))?;
        }
        OutputFormat::VisualFile { visual_output } => {
            let html = build_visual_report_html(
                report_table_full,
                &cfg.category_group_watch_list,
                &visual_week_label,
                week_year,
                cfg.show_all_rows,
            )?;
            std::fs::write(visual_output, &html)
                .with_context(|| format!("writing {visual_output:?}"))?;
        }
    }

    Ok(())
}

fn write_csv_string(df: &mut DataFrame) -> Result<String> {
    let mut buf = Vec::new();
    CsvWriter::new(&mut buf)
        .finish(df)
        .context("writing CSV")?;
    String::from_utf8(buf).context("CSV not valid UTF-8")
}

fn format_short_date(date: chrono::NaiveDate) -> String {
    let formatted = date.format("%b %d").to_string();
    if let Some(space_pos) = formatted.rfind(' ') {
        let (prefix, day_part) = formatted.split_at(space_pos + 1);
        if day_part.starts_with('0') {
            return format!("{}{}", prefix, &day_part[1..]);
        }
    }
    formatted
}

fn main() -> Result<()> {
    let args = Args::parse();
    let cfg = config::load_config(&args.config)?;
    let api = HttpYnabClient::new(&cfg.personal_access_token)?;
    run(&api, &cfg)
}
