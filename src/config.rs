use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::NaiveDate;
use indexmap::IndexMap;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub enum SimpleOutputFormat {
    #[serde(rename = "polars_print")]
    PolarsPrint,
    #[serde(rename = "csv_print")]
    CsvPrint,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum OutputFormat {
    Simple(SimpleOutputFormat),
    CsvFile { csv_output: PathBuf },
    VisualFile { visual_output: PathBuf },
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Simple(SimpleOutputFormat::PolarsPrint)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub budget_name: String,
    pub personal_access_token: String,
    pub category_group_watch_list: IndexMap<String, String>,
    #[serde(default)]
    pub resolution_date: Option<NaiveDate>,
    #[serde(default)]
    pub show_all_rows: bool,
    #[serde(default)]
    pub output_format: OutputFormat,
}

pub fn load_config(path: &Path) -> Result<Config> {
    let contents =
        std::fs::read_to_string(path).with_context(|| format!("reading config from {path:?}"))?;
    serde_json::from_str(&contents).with_context(|| "parsing config JSON")
}
