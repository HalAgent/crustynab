use anyhow::{Context, Result, bail};
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;

// --- API response types ---

#[derive(Debug, Clone, Deserialize)]
pub struct BudgetSummary {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub category_group_name: Option<String>,
    #[serde(default)]
    pub budgeted: i64,
    #[serde(default)]
    pub balance: i64,
    #[serde(default)]
    pub goal_cadence: Option<i32>,
    #[serde(default)]
    pub goal_target: Option<i64>,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CategoryGroup {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub categories: Vec<Category>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubTransaction {
    #[serde(default)]
    pub amount: i64,
    #[serde(default)]
    pub payee_name: Option<String>,
    #[serde(default)]
    pub category_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub date: NaiveDate,
    #[serde(default)]
    pub amount: i64,
    #[serde(default)]
    pub payee_name: Option<String>,
    #[serde(default)]
    pub category_name: Option<String>,
    #[serde(default)]
    pub subtransactions: Vec<SubTransaction>,
}

// --- API response envelopes ---

#[derive(Debug, Deserialize)]
struct BudgetsResponseData {
    budgets: Vec<BudgetSummary>,
}

#[derive(Debug, Deserialize)]
struct BudgetsResponse {
    data: BudgetsResponseData,
}

#[derive(Debug, Deserialize)]
struct CategoriesResponseData {
    category_groups: Vec<CategoryGroup>,
}

#[derive(Debug, Deserialize)]
struct CategoriesResponse {
    data: CategoriesResponseData,
}

#[derive(Debug, Deserialize)]
struct CategoryResponseData {
    category: Category,
}

#[derive(Debug, Deserialize)]
struct CategoryResponse {
    data: CategoryResponseData,
}

#[derive(Debug, Deserialize)]
struct TransactionsResponseData {
    transactions: Vec<Transaction>,
}

#[derive(Debug, Deserialize)]
struct TransactionsResponse {
    data: TransactionsResponseData,
}

// --- Client trait ---

pub trait YnabApi {
    fn get_budgets(&self) -> Result<Vec<BudgetSummary>>;
    fn get_category_groups(&self, budget_id: &str) -> Result<Vec<CategoryGroup>>;
    fn get_month_category(
        &self,
        budget_id: &str,
        month: NaiveDate,
        category_id: &str,
    ) -> Result<Category>;
    fn get_transactions(
        &self,
        budget_id: &str,
        since_date: NaiveDate,
    ) -> Result<Vec<Transaction>>;
}

// --- HTTP implementation ---

const BASE_URL: &str = "https://api.ynab.com/v1";

pub struct HttpYnabClient {
    client: reqwest::blocking::Client,
    token: String,
}

impl HttpYnabClient {
    pub fn new(token: &str) -> Result<Self> {
        let client = reqwest::blocking::Client::new();
        Ok(Self {
            client,
            token: token.to_string(),
        })
    }

    fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let resp = self
            .client
            .get(url)
            .bearer_auth(&self.token)
            .send()
            .with_context(|| format!("GET {url}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            bail!("YNAB API returned {status} for {url}: {body}");
        }

        resp.json::<T>()
            .with_context(|| format!("parsing response from {url}"))
    }
}

impl YnabApi for HttpYnabClient {
    fn get_budgets(&self) -> Result<Vec<BudgetSummary>> {
        let resp: BudgetsResponse =
            self.get_json(&format!("{BASE_URL}/budgets"))?;
        Ok(resp.data.budgets)
    }

    fn get_category_groups(&self, budget_id: &str) -> Result<Vec<CategoryGroup>> {
        let resp: CategoriesResponse =
            self.get_json(&format!("{BASE_URL}/budgets/{budget_id}/categories"))?;
        Ok(resp.data.category_groups)
    }

    fn get_month_category(
        &self,
        budget_id: &str,
        month: NaiveDate,
        category_id: &str,
    ) -> Result<Category> {
        let first_of_month =
            NaiveDate::from_ymd_opt(month.year(), month.month(), 1)
                .ok_or_else(|| anyhow::anyhow!("invalid month from {month}"))?;
        let month_str = first_of_month.format("%Y-%m-%d");
        let resp: CategoryResponse = self.get_json(&format!(
            "{BASE_URL}/budgets/{budget_id}/months/{month_str}/categories/{category_id}"
        ))?;
        Ok(resp.data.category)
    }

    fn get_transactions(
        &self,
        budget_id: &str,
        since_date: NaiveDate,
    ) -> Result<Vec<Transaction>> {
        let resp: TransactionsResponse = self.get_json(&format!(
            "{BASE_URL}/budgets/{budget_id}/transactions?since_date={since_date}"
        ))?;
        Ok(resp.data.transactions)
    }
}
