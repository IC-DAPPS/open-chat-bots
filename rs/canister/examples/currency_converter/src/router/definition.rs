use super::commands;
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk_canister::{HttpRequest, HttpResponse};

pub async fn get(_request: HttpRequest) -> HttpResponse {
    HttpResponse::json(
        200,
        &BotDefinition {
            description: "A currency converter bot that provides real-time exchange rates between different currencies. Supports all ISO standard currencies and formats numbers for easy readability. Uses the XRC (Exchange Rate Canister) as its data source.".to_string(),
            commands: commands::definitions(),
            autonomous_config: None,
        },
    )
}
