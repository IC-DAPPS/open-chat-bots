use super::commands;
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk_canister::{HttpRequest, HttpResponse};

pub async fn get(_request: HttpRequest) -> HttpResponse {
    HttpResponse::json(
        200,
        &BotDefinition {
            description:
            "PriceBot provides real-time cryptocurrency and fiat currency exchange rates from multiple sources.
            Get up-to-date price information for cryptocurrencies via ICPSwap or exchange rates between any currency
            pairs (crypto or fiat) using the Exchange Rate Canister. Administrators can configure the bot to track
            specific assets for their community.".to_string(),
            commands: commands::definitions(),
            autonomous_config: None,
        },
    )
}
