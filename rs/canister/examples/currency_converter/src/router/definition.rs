use super::commands;
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk_canister::{HttpRequest, HttpResponse};

pub async fn get(_request: HttpRequest) -> HttpResponse {
    HttpResponse::json(
        200,
        &BotDefinition {
            description: "A currency converter bot that provides real-time exchange rates between different currencies. Supports all ISO standard fiat currencies and popular cryptocurrencies with easy conversion between them. Uses the XRC (Exchange Rate Canister) as its data source.".to_string(),
            commands: commands::definitions(),
            autonomous_config: None,
        },
    )
}
