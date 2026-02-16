use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate};
use futures::executor::block_on;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use ynab_api::apis::configuration::{ApiKey, Configuration};
use ynab_api::apis::{budgets_api, categories_api, transactions_api};

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
    fn get_transactions(&self, budget_id: &str, since_date: NaiveDate) -> Result<Vec<Transaction>>;
}

// --- HTTP implementation ---

pub struct HttpYnabClient {
    configuration: Configuration,
}

impl HttpYnabClient {
    pub fn new(token: &str) -> Result<Self> {
        let mut configuration = Configuration::new();
        configuration.api_key = Some(ApiKey {
            prefix: Some("Bearer".to_string()),
            key: token.to_string(),
        });

        Ok(Self { configuration })
    }

    fn map_model<TSrc, TDst>(&self, src: TSrc, name: &str) -> Result<TDst>
    where
        TSrc: Serialize,
        TDst: DeserializeOwned,
    {
        let value = serde_json::to_value(src)
            .with_context(|| format!("serializing YNAB response model {name}"))?;
        serde_json::from_value(value)
            .with_context(|| format!("deserializing YNAB response model into crustynab {name}"))
    }
}

impl YnabApi for HttpYnabClient {
    fn get_budgets(&self) -> Result<Vec<BudgetSummary>> {
        let response = block_on(budgets_api::get_budgets(&self.configuration, None))
            .map_err(|err| anyhow::anyhow!("get_budgets failed: {err:?}"))?;
        let resp: BudgetsResponse = self.map_model(response, "BudgetSummaryResponse")?;
        Ok(resp.data.budgets)
    }

    fn get_category_groups(&self, budget_id: &str) -> Result<Vec<CategoryGroup>> {
        let response = block_on(categories_api::get_categories(
            &self.configuration,
            budget_id,
            None,
        ))
        .map_err(|err| anyhow::anyhow!("get_categories failed for budget {budget_id}: {err:?}"))?;
        let resp: CategoriesResponse = self.map_model(response, "CategoriesResponse")?;
        Ok(resp.data.category_groups)
    }

    fn get_month_category(
        &self,
        budget_id: &str,
        month: NaiveDate,
        category_id: &str,
    ) -> Result<Category> {
        let first_of_month = NaiveDate::from_ymd_opt(month.year(), month.month(), 1)
            .ok_or_else(|| anyhow::anyhow!("invalid month from {month}"))?;
        let month_str = first_of_month.format("%Y-%m-%d").to_string();
        let response = block_on(categories_api::get_month_category_by_id(
            &self.configuration,
            budget_id,
            month_str.clone(),
            category_id,
        ))
        .map_err(|err| {
            anyhow::anyhow!(
                "get_month_category_by_id failed for budget {budget_id}, month {month_str}, category {category_id}: {err:?}"
            )
        })?;
        let resp: CategoryResponse = self.map_model(response, "CategoryResponse")?;
        Ok(resp.data.category)
    }

    fn get_transactions(&self, budget_id: &str, since_date: NaiveDate) -> Result<Vec<Transaction>> {
        let since = since_date.format("%Y-%m-%d").to_string();
        let response = block_on(transactions_api::get_transactions(
            &self.configuration,
            budget_id,
            Some(since.clone()),
            None,
            None,
        ))
        .map_err(|err| {
            anyhow::anyhow!(
                "get_transactions failed for budget {budget_id}, since_date {since}: {err:?}"
            )
        })?;
        let resp: TransactionsResponse = self.map_model(response, "TransactionsResponse")?;
        Ok(resp.data.transactions)
    }
}
