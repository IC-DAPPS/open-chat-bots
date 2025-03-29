use candid::{CandidType, Principal};
use ic_cdk::api::call::{call_with_payment, msg_cycles_refunded, CallResult};
use serde::{Deserialize, Serialize};

#[derive(CandidType, Serialize, Deserialize, Debug, Clone)]
pub enum AssetClass {
    Cryptocurrency,
    FiatCurrency,
}

impl AssetClass {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "Cryptocurrency" => Ok(Self::Cryptocurrency),
            "FiatCurrency" => Ok(Self::FiatCurrency),
            _ => Err(format!("Invalid asset class: {}", s)),
        }
    }
}

impl ToString for AssetClass {
    fn to_string(&self) -> String {
        match self {
            Self::Cryptocurrency => "Cryptocurrency".to_string(),
            Self::FiatCurrency => "FiatCurrency".to_string(),
        }
    }
}

#[derive(CandidType, Serialize, Deserialize, Debug, Clone)]
pub struct Asset {
    pub class: AssetClass,
    pub symbol: String,
}

impl Asset {
    pub fn new(class: AssetClass, symbol: String) -> Self {
        Self { class, symbol }
    }
    pub fn new_from_strings(class: &str, symbol: String) -> Result<Self, String> {
        match AssetClass::from_str(class) {
            Ok(asset_class) => Ok(Self::new(asset_class, symbol)),
            Err(err) => Err(err),
        }
    }
}

#[derive(CandidType, Deserialize)]
pub struct GetExchangeRateRequest {
    pub timestamp: Option<u64>,
    pub quote_asset: Asset,
    pub base_asset: Asset,
}

#[derive(CandidType, Deserialize)]
pub struct ExchangeRateMetadata {
    pub decimals: u32,
    pub forex_timestamp: Option<u64>,
    pub quote_asset_num_received_rates: u64,
    pub base_asset_num_received_rates: u64,
    pub base_asset_num_queried_sources: u64,
    pub standard_deviation: u64,
    pub quote_asset_num_queried_sources: u64,
}

#[derive(CandidType, Deserialize)]
pub struct ExchangeRate {
    pub metadata: ExchangeRateMetadata,
    pub rate: u64,
    pub timestamp: u64,
    pub quote_asset: Asset,
    pub base_asset: Asset,
}

#[derive(CandidType, Deserialize, Debug)]
pub enum ExchangeRateError {
    AnonymousPrincipalNotAllowed,
    CryptoQuoteAssetNotFound,
    FailedToAcceptCycles,
    ForexBaseAssetNotFound,
    CryptoBaseAssetNotFound,
    StablecoinRateTooFewRates,
    ForexAssetsNotFound,
    InconsistentRatesReceived,
    RateLimited,
    StablecoinRateZeroRate,
    Other { code: u32, description: String },
    ForexInvalidTimestamp,
    NotEnoughCycles,
    ForexQuoteAssetNotFound,
    StablecoinRateNotFound,
    Pending,
}

use std::fmt;

impl fmt::Display for ExchangeRateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExchangeRateError::AnonymousPrincipalNotAllowed => {
                write!(f, "Anonymous Principal Not Allowed")
            }
            ExchangeRateError::CryptoQuoteAssetNotFound => {
                write!(f, "Crypto Quote Asset Not Found")
            }
            ExchangeRateError::FailedToAcceptCycles => write!(f, "Failed To Accept Cycles"),
            ExchangeRateError::ForexBaseAssetNotFound => write!(f, "Forex Base Asset Not Found"),
            ExchangeRateError::CryptoBaseAssetNotFound => write!(f, "Crypto Base Asset Not Found"),
            ExchangeRateError::StablecoinRateTooFewRates => {
                write!(f, "Stablecoin Rate Too Few Rates")
            }
            ExchangeRateError::ForexAssetsNotFound => write!(f, "Forex Assets Not Found"),
            ExchangeRateError::InconsistentRatesReceived => {
                write!(f, "Inconsistent Rates Received")
            }
            ExchangeRateError::RateLimited => write!(f, "Rate Limited"),
            ExchangeRateError::StablecoinRateZeroRate => write!(f, "Stablecoin Rate Zero Rate"),
            ExchangeRateError::Other { code, description } => {
                write!(f, "Other: Code {}, Description {}", code, description)
            }
            ExchangeRateError::ForexInvalidTimestamp => write!(f, "Forex Invalid Timestamp"),
            ExchangeRateError::NotEnoughCycles => write!(f, "Not Enough Cycles"),
            ExchangeRateError::ForexQuoteAssetNotFound => write!(f, "Forex Quote Asset Not Found"),
            ExchangeRateError::StablecoinRateNotFound => write!(f, "Stablecoin Rate Not Found"),
            ExchangeRateError::Pending => write!(f, "Pending"),
        }
    }
}

#[derive(CandidType, Deserialize)]
pub enum GetExchangeRateResult {
    Ok(ExchangeRate),
    Err(ExchangeRateError),
}

pub async fn get_exchange_rate(
    arg0: GetExchangeRateRequest,
) -> (CallResult<(GetExchangeRateResult,)>, u64) {
    let xrc_canister = Principal::from_text("uf6dk-hyaaa-aaaaq-qaaaq-cai".to_string()).unwrap();

    let result =
        call_with_payment(xrc_canister, "get_exchange_rate", (arg0,), 1_000_000_000u64).await;

    (result, msg_cycles_refunded())
}

pub async fn get_latest_price(base_asset: Asset, quote_asset: Asset) -> Result<(f64, u64), String> {
    let xrc_arg = GetExchangeRateRequest {
        base_asset,
        quote_asset,
        timestamp: None,
    };

    let (call_result, _cycles_refunded) = get_exchange_rate(xrc_arg).await;
    let (get_xrate_result,) =
        call_result.map_err(|e| format!("Failed to get Price. XRC call failed: {:?}", e))?;

    let xrate = match get_xrate_result {
        GetExchangeRateResult::Ok(rate) => rate,
        GetExchangeRateResult::Err(err) => {
            return Err(format!("Failed to get Price. XRC returned {}", err))
        }
    };

    let timestamp = xrate.timestamp;

    Ok((
        get_price_from_rate(xrate),
        get_expiration_time_xrc(timestamp),
    ))
}

fn get_expiration_time_xrc(timestamp_in_sec: u64) -> u64 {
    let a_minute_and_half = 90;
    let nanosec = 1_000_000_000;

    (timestamp_in_sec + a_minute_and_half) * nanosec
}

fn get_price_from_rate(xrate: ExchangeRate) -> f64 {
    let divisor: u64 = 10_u64.pow(xrate.metadata.decimals);
    (xrate.rate as f64) / (divisor as f64)
}

/*
    if timestamp is Null
    the timeof new rate i request is at 1743157530
    timestamp in the xrate response is for example 1743157500 .

    my assumption is that if i pass timestamp as null. the rate i received is from the cache of the xrc canister. which is  20M cycles.

    from this i understand that rate at 1743157500 is cached at 1743157530. which is cached 30 seconds after the latest rate.

    that means next latest rate will be available at 1743157560 but will be cached at 1743157590.
*/
