use candid::{CandidType, Int, Nat, Principal};
use ic_cdk::api::call::CallResult;
use ic_cdk::call;
use serde::Deserialize;

pub async fn get_icrc_ledger_name(ledger_id: Principal) -> CallResult<(String,)> {
    call(ledger_id, "icrc1_name", ()).await
}

async fn token_storage(canister_id: Principal, arg0: String) -> CallResult<(Option<String>,)> {
    ic_cdk::call(canister_id, "tokenStorage", (arg0,)).await
}

#[derive(CandidType, Deserialize)]
pub struct PublicTokenPricesData {
    pub id: candid::Int,
    pub low: f64,
    pub high: f64,
    pub close: f64,
    pub open: f64,
    pub timestamp: candid::Int,
}

async fn get_token_prices_data(
    canister_id: Principal,
    arg0: String,
    arg1: Int,
    arg2: Int,
    arg3: Nat,
) -> CallResult<(Vec<PublicTokenPricesData>,)> {
    ic_cdk::call(canister_id, "getTokenPricesData", (arg0, arg1, arg2, arg3)).await
}

pub async fn get_latest_price(ledger_id: Principal) -> Result<f64, String> {
    let node_index = Principal::from_text("ggzvv-5qaaa-aaaag-qck7a-cai").unwrap();

    let (opt_token_storage,) = token_storage(node_index, ledger_id.to_string())
        .await
        .map_err(|e| format!("Failed to get token storage: {:?}", e))?;

    let token_storage = match opt_token_storage {
        Some(canister_id) => Principal::from_text(canister_id).unwrap(),
        None => return Err(format!("Failed to get Price. Token storage not found.")),
    };

    let (pb_token_prices,) = get_token_prices_data(
        token_storage,
        ledger_id.to_string(),
        Int::from(0),
        Int::from(86400),
        Nat::from(1u32),
    )
    .await
    .map_err(|e| format!("Failed to get Price. {:?}", e))?;

    let latest_price = pb_token_prices
        .first()
        .ok_or(format!("Failed to get Price. No data found."))?;

    Ok(latest_price.close)
}
