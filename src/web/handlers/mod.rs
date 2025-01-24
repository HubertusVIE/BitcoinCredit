use super::data::{
    BalanceResponse, CurrenciesResponse, CurrencyResponse, OverviewBalanceResponse,
    OverviewResponse,
};
use crate::{
    constants::VALID_CURRENCIES,
    service::{Error, Result, ServiceContext},
};
use rocket::{get, serde::json::Json, Shutdown, State};

pub mod bill;
pub mod company;
pub mod contacts;
pub mod identity;
pub mod middleware;
pub mod notifications;
pub mod quotes;

#[get("/")]
pub async fn exit(shutdown: Shutdown, state: &State<ServiceContext>) {
    log::info!("Exit called - shutting down...");
    shutdown.notify();
    state.shutdown();
}

#[get("/")]
pub async fn currencies(_state: &State<ServiceContext>) -> Result<Json<CurrenciesResponse>> {
    Ok(Json(CurrenciesResponse {
        currencies: VALID_CURRENCIES
            .iter()
            .map(|vc| CurrencyResponse {
                code: vc.to_string(),
            })
            .collect(),
    }))
}

#[get("/?<currency>")]
pub async fn overview(
    currency: &str,
    state: &State<ServiceContext>,
) -> Result<Json<OverviewResponse>> {
    if !VALID_CURRENCIES.contains(&currency) {
        return Err(Error::Validation(format!(
            "Currency with code '{}' not found",
            currency
        )));
    }
    let result = state.bill_service.get_bill_balances(currency).await?;

    Ok(Json(OverviewResponse {
        currency: currency.to_owned(),
        balances: OverviewBalanceResponse {
            payee: BalanceResponse {
                sum: result.payee.sum,
            },
            payer: BalanceResponse {
                sum: result.payer.sum,
            },
            contingent: BalanceResponse {
                sum: result.contingent.sum,
            },
        },
    }))
}
