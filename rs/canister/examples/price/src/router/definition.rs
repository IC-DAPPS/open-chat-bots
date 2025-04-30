use super::commands;
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk_canister::{HttpRequest, HttpResponse};

pub async fn get(_request: HttpRequest) -> HttpResponse {
    HttpResponse::json(
        200,
        &BotDefinition {
            description:
            "Price Bot is a bot that shows the price of a crypto/fiat currency. Each community or group can configure the bot to get the price of any crypto/fiat currency according to their needs.".to_string(),
            commands: commands::definitions(),
            autonomous_config: None,
        },
    )
}
