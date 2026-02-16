use anyhow::{Context, Result};
use indexmap::IndexMap;
use polars::prelude::*;

pub const CURRENCY: &str = "£";

pub fn format_currency(value: f64, show_zero: bool) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded == 0.0 && !show_zero {
        return String::new();
    }
    let sign = if rounded < 0.0 { "-" } else { "" };
    let abs_val = rounded.abs();
    format!("{sign}{CURRENCY}{}", format_with_commas(abs_val))
}

fn format_with_commas(value: f64) -> String {
    let formatted = format!("{:.2}", value);
    let (integer_part, decimal_part) = formatted.split_once('.').unwrap_or((&formatted, "00"));

    let chars: Vec<char> = integer_part.chars().collect();
    let with_commas: String = chars
        .iter()
        .rev()
        .enumerate()
        .fold(Vec::new(), |mut acc, (i, &c)| {
            if i > 0 && i % 3 == 0 {
                acc.push(',');
            }
            acc.push(c);
            acc
        })
        .into_iter()
        .rev()
        .collect();

    format!("{with_commas}.{decimal_part}")
}

pub fn darken_hex(color: &str, factor: f64) -> String {
    if !color.starts_with('#') || color.len() != 7 {
        return color.to_string();
    }
    let parse = || -> Option<String> {
        let r = u8::from_str_radix(&color[1..3], 16).ok()?;
        let g = u8::from_str_radix(&color[3..5], 16).ok()?;
        let b = u8::from_str_radix(&color[5..7], 16).ok()?;
        let dr = (r as f64 * factor) as u8;
        let dg = (g as f64 * factor) as u8;
        let db = (b as f64 * factor) as u8;
        Some(format!("#{dr:02x}{dg:02x}{db:02x}"))
    };
    parse().unwrap_or_else(|| color.to_string())
}

fn with_value_columns(df: &DataFrame) -> Result<DataFrame> {
    let is_annual = df
        .column("goal_cadence")
        .context("goal_cadence column")?
        .str()
        .context("goal_cadence as str")?
        .equal("annual");

    let budgeted = df
        .column("budgeted")
        .context("budgeted column")?
        .f64()
        .context("budgeted as f64")?;

    let planned: Float64Chunked = is_annual
        .iter()
        .zip(budgeted.iter())
        .map(|(is_ann, bud)| match (is_ann, bud) {
            (Some(true), Some(b)) => Some(b),
            (Some(false), Some(b)) => Some(b * 12.0),
            _ => None,
        })
        .collect();

    let per_month: Float64Chunked = planned.iter().map(|p| p.map(|v| v / 12.0)).collect();

    let remaining = df.column("balance").context("balance column")?.clone();

    let is_annual_bool: BooleanChunked = is_annual;

    let mut result = df.clone();
    let planned_series = planned.with_name("planned".into()).into_series();
    let per_month_series = per_month.with_name("per_month".into()).into_series();
    let is_annual_series = is_annual_bool.with_name("is_annual".into()).into_series();

    result
        .with_column(planned_series)
        .context("adding planned")?;
    result
        .with_column(per_month_series)
        .context("adding per_month")?;
    result
        .with_column(remaining.with_name("remaining".into()))
        .context("adding remaining")?;
    result
        .with_column(is_annual_series)
        .context("adding is_annual")?;

    Ok(result)
}

struct RowData {
    category: String,
    planned: f64,
    per_month: f64,
    spent: f64,
    remaining: f64,
    color: String,
    is_total: bool,
    show_period_values: bool,
    is_annual: bool,
}

fn row_html(data: &RowData) -> String {
    let class_name = if data.is_total { "total" } else { "group" };
    let row_style = format!(" style=\"background-color: {};\"", data.color);
    let show_values = data.show_period_values || data.is_total;

    let annual_style = if data.is_annual {
        format!(
            " style=\"background-color: {};\"",
            darken_hex(&data.color, 0.7)
        )
    } else {
        String::new()
    };

    let remaining_value = if data.is_total || !show_values {
        String::new()
    } else {
        format_currency(data.remaining, show_values)
    };

    let escaped_category = html_escape::encode_quoted_attribute(&data.category);

    [
        format!(r#"      <tr class="{class_name}"{row_style}>"#),
        format!("        <td>{escaped_category}</td>"),
        format!(
            r#"        <td class="number"{annual_style}>{}</td>"#,
            format_currency(data.planned, data.is_total)
        ),
        format!(
            r#"        <td class="number"{annual_style}>{}</td>"#,
            format_currency(data.per_month, data.is_total)
        ),
        format!(
            r#"        <td class="number">{}</td>"#,
            format_currency(-data.spent, show_values)
        ),
        format!(r#"        <td class="number">{remaining_value}</td>"#),
        "      </tr>".to_string(),
    ]
    .join("\n")
}

pub fn build_visual_report_html(
    report_table: LazyFrame,
    group_colors: &IndexMap<String, String>,
    week_label: &str,
    planned_year: i32,
    show_all_rows: bool,
) -> Result<String> {
    let report_df = report_table
        .collect()
        .context("collecting report table for visual")?;

    let display_df = if show_all_rows {
        report_df.clone()
    } else {
        report_df
            .clone()
            .lazy()
            .filter(col("spent").neq(lit(0.0)))
            .collect()
            .context("filtering display rows")?
    };

    let mut rows: Vec<String> = Vec::new();
    let mut total_planned = 0.0_f64;
    let mut total_per_month = 0.0_f64;
    let mut total_spent = 0.0_f64;
    let mut total_remaining = 0.0_f64;

    for (group_name, color) in group_colors {
        let group_df = report_df
            .clone()
            .lazy()
            .filter(col("category_group_name").eq(lit(group_name.as_str())))
            .sort(["category_name"], SortMultipleOptions::default())
            .collect()
            .context("filtering group")?;

        let display_group_df = display_df
            .clone()
            .lazy()
            .filter(col("category_group_name").eq(lit(group_name.as_str())))
            .sort(["category_name"], SortMultipleOptions::default())
            .collect()
            .context("filtering display group")?;

        if group_df.is_empty() {
            continue;
        }

        let group_values = with_value_columns(&group_df)?;
        let display_values = with_value_columns(&display_group_df)?;

        let group_planned: f64 = group_values
            .column("planned")
            .context("planned col")?
            .as_materialized_series()
            .f64()
            .context("planned f64")?
            .sum()
            .unwrap_or(0.0);
        let group_per_month: f64 = group_values
            .column("per_month")
            .context("per_month col")?
            .as_materialized_series()
            .f64()
            .context("per_month f64")?
            .sum()
            .unwrap_or(0.0);
        let group_spent: f64 = group_values
            .column("spent")
            .context("spent col")?
            .as_materialized_series()
            .f64()
            .context("spent f64")?
            .sum()
            .unwrap_or(0.0);
        let group_remaining: f64 = group_values
            .column("remaining")
            .context("remaining col")?
            .as_materialized_series()
            .f64()
            .context("remaining f64")?
            .sum()
            .unwrap_or(0.0);

        total_planned += group_planned;
        total_per_month += group_per_month;
        total_spent += group_spent;
        total_remaining += group_remaining;

        for i in 0..display_values.height() {
            let cat_name = display_values
                .column("category_name")
                .context("cat name")?
                .str()
                .context("cat name str")?
                .get(i)
                .unwrap_or("");
            let planned: f64 = display_values
                .column("planned")
                .context("planned")?
                .f64()
                .context("planned f64")?
                .get(i)
                .unwrap_or(0.0);
            let per_month: f64 = display_values
                .column("per_month")
                .context("per_month")?
                .f64()
                .context("per_month f64")?
                .get(i)
                .unwrap_or(0.0);
            let spent: f64 = display_values
                .column("spent")
                .context("spent")?
                .f64()
                .context("spent f64")?
                .get(i)
                .unwrap_or(0.0);
            let remaining: f64 = display_values
                .column("remaining")
                .context("remaining")?
                .f64()
                .context("remaining f64")?
                .get(i)
                .unwrap_or(0.0);
            let is_annual = display_values
                .column("is_annual")
                .context("is_annual")?
                .bool()
                .context("is_annual bool")?
                .get(i)
                .unwrap_or(false);

            rows.push(row_html(&RowData {
                category: cat_name.to_string(),
                planned,
                per_month,
                spent,
                remaining,
                color: color.clone(),
                is_total: false,
                show_period_values: spent != 0.0,
                is_annual,
            }));
        }

        rows.push(row_html(&RowData {
            category: format!("Total {group_name}"),
            planned: group_planned,
            per_month: group_per_month,
            spent: group_spent,
            remaining: group_remaining,
            color: darken_hex(color, 0.85),
            is_total: true,
            show_period_values: true,
            is_annual: false,
        }));
    }

    if !rows.is_empty() {
        rows.push(row_html(&RowData {
            category: "Total".to_string(),
            planned: total_planned,
            per_month: total_per_month,
            spent: total_spent,
            remaining: total_remaining,
            color: "#b7b7b7".to_string(),
            is_total: true,
            show_period_values: true,
            is_annual: false,
        }));
    }

    let body_rows = rows.join("\n");
    let escaped_week = html_escape::encode_text(week_label);

    let html = [
        "<!DOCTYPE html>",
        r#"<html lang="en">"#,
        "<head>",
        r#"  <meta charset="utf-8">"#,
        r#"  <meta name="viewport" content="width=device-width, initial-scale=1">"#,
        "  <title>Budget Visual Report</title>",
        "  <style>",
        "    :root {",
        "      --grid: #d9d9d9;",
        "      --header-bg: #f7f3e9;",
        "      --text: #1f1f1f;",
        "    }",
        "    body {",
        "      margin: 24px;",
        r#"      font-family: "Alegreya Sans", "Trebuchet MS", sans-serif;"#,
        "      color: var(--text);",
        "      background: linear-gradient(180deg, #fbf9f4 0%, #f3efe7 100%);",
        "      -webkit-user-select: text;",
        "      user-select: text;",
        "    }",
        "    h1 {",
        "      font-size: 20px;",
        "      margin: 0 0 16px 0;",
        "      letter-spacing: 0.02em;",
        "      text-transform: uppercase;",
        "    }",
        "    table {",
        "      width: 100%;",
        "      border-collapse: collapse;",
        "      background: #fffefc;",
        "      box-shadow: 0 6px 24px rgba(0, 0, 0, 0.08);",
        "      user-select: none;",
        "    }",
        "    th, td {",
        "      border: 1px solid var(--grid);",
        "      padding: 6px 8px;",
        "      font-size: 13px;",
        "      vertical-align: middle;",
        "      -webkit-user-select: text;",
        "      user-select: text;",
        "    }",
        "    th {",
        "      background: var(--header-bg);",
        "      text-align: left;",
        "      font-weight: 700;",
        "    }",
        "    td.number {",
        "      text-align: right;",
        "      white-space: nowrap;",
        "    }",
        "    tr.total td {",
        "      font-weight: 700;",
        "      border-top: 2px solid #9a9a9a;",
        "    }",
        "    td.selected {",
        "      outline: 2px solid #2a5d86;",
        "      outline-offset: -2px;",
        "      position: relative;",
        "    }",
        "    @media (max-width: 760px) {",
        "      body { margin: 12px; }",
        "      th, td { font-size: 12px; }",
        "    }",
        "  </style>",
        "</head>",
        "<body>",
        &format!("  <h1>{escaped_week}</h1>"),
        r#"  <table class="selectable">"#,
        "    <thead>",
        "      <tr>",
        r#"        <th rowspan="2">Category</th>"#,
        &format!(r#"        <th rowspan="2">{planned_year} (planned)</th>"#),
        &format!(r#"        <th rowspan="2">{planned_year} per month</th>"#),
        &format!(r#"        <th colspan="2">{escaped_week}</th>"#),
        "      </tr>",
        "      <tr>",
        "        <th>Spent</th>",
        "        <th>Remaining in period</th>",
        "      </tr>",
        "    </thead>",
        "    <tbody>",
        &body_rows,
        "    </tbody>",
        "  </table>",
        "  <script>",
        r#"    const table = document.querySelector("table.selectable");"#,
        "    if (table) {",
        r#"      const rows = Array.from(table.querySelectorAll("tbody tr"));"#,
        "      const cellGrid = rows.map((row, rowIndex) => {",
        r#"        return Array.from(row.querySelectorAll("td")).map((cell, colIndex) => {"#,
        "          cell.dataset.row = String(rowIndex);",
        "          cell.dataset.col = String(colIndex);",
        "          return cell;",
        "        });",
        "      });",
        "      let selecting = false;",
        "      let startCell = null;",
        "      let selection = null;",
        "      const clearSelection = () => {",
        r#"        table.querySelectorAll("td.selected").forEach((cell) => {"#,
        r#"          cell.classList.remove("selected");"#,
        "        });",
        "      };",
        "      const applySelection = (endCell) => {",
        "        if (!startCell || !endCell) {",
        "          return;",
        "        }",
        "        const startRow = Number(startCell.dataset.row);",
        "        const startCol = Number(startCell.dataset.col);",
        "        const endRow = Number(endCell.dataset.row);",
        "        const endCol = Number(endCell.dataset.col);",
        "        const minRow = Math.min(startRow, endRow);",
        "        const maxRow = Math.max(startRow, endRow);",
        "        const minCol = Math.min(startCol, endCol);",
        "        const maxCol = Math.max(startCol, endCol);",
        "        selection = { minRow, maxRow, minCol, maxCol };",
        "        clearSelection();",
        "        for (let row = minRow; row <= maxRow; row += 1) {",
        "          const cells = cellGrid[row] || [];",
        "          for (let col = minCol; col <= maxCol; col += 1) {",
        "            const cell = cells[col];",
        "            if (cell) {",
        r#"              cell.classList.add("selected");"#,
        "            }",
        "          }",
        "        }",
        "      };",
        r#"      table.addEventListener("mousedown", (event) => {"#,
        r#"        const cell = event.target.closest("td");"#,
        "        if (!cell) {",
        "          return;",
        "        }",
        "        selecting = true;",
        "        startCell = cell;",
        "        applySelection(cell);",
        "        event.preventDefault();",
        "      });",
        r#"      table.addEventListener("mouseover", (event) => {"#,
        "        if (!selecting) {",
        "          return;",
        "        }",
        r#"        const cell = event.target.closest("td");"#,
        "        if (cell) {",
        "          applySelection(cell);",
        "        }",
        "      });",
        r#"      document.addEventListener("mouseup", () => {"#,
        "        selecting = false;",
        "      });",
        r#"      document.addEventListener("copy", (event) => {"#,
        "        if (!selection) {",
        "          return;",
        "        }",
        "        const { minRow, maxRow, minCol, maxCol } = selection;",
        "        const lines = [];",
        "        for (let row = minRow; row <= maxRow; row += 1) {",
        "          const cells = cellGrid[row] || [];",
        "          const values = [];",
        "          for (let col = minCol; col <= maxCol; col += 1) {",
        "            const cell = cells[col];",
        r#"            values.push(cell ? cell.innerText.trim() : "");"#,
        "          }",
        r#"          lines.push(values.join("\t"));"#,
        "        }",
        r#"        event.clipboardData.setData("text/plain", lines.join("\n"));"#,
        "        event.preventDefault();",
        "      });",
        "    }",
        "  </script>",
        "</body>",
        "</html>",
    ];

    Ok(format!("{}\n", html.join("\n")))
}
