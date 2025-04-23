use super::commands;
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk_canister::{HttpRequest, HttpResponse};

pub async fn get(_request: HttpRequest) -> HttpResponse {
    HttpResponse::json(
        200,
        &BotDefinition {
            description:
            "A bot to preload with answers to frequently asked questions (FAQ), easily configurable for each community".to_string(),
            commands: commands::definitions(),
            autonomous_config: None,
        },
    )
}
