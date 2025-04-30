use super::commands;
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk_canister::{HttpRequest, HttpResponse};

pub async fn get(_request: HttpRequest) -> HttpResponse {
    HttpResponse::json(
        200,
        &BotDefinition {
            description:
                "A utility bot providing low-level Internet Computer functionality including canister management, account/principal validation, subnet lookup, and cycle balance checks"
                    .to_string(),
            commands: commands::definitions(),
            autonomous_config: None,
        },
    )
}
