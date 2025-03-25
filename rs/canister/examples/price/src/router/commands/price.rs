use async_trait::async_trait;
use oc_bots_sdk::api::command::{CommandHandler, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::actions::send_message;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::BotCommandContext;
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(Price::definition);

pub struct Price;

#[async_trait]
impl CommandHandler<CanisterRuntime> for Price {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        let url = "https://internetcomputer.org/img/IC_logo_horizontal_white.svg";
        // let element = format!(
        //     "<img src=\"{}\" alt=\"Example image\" width=\"48\" height=\"48\"/>",
        //     url
        // );
        let user_id = oc_client.context().command.initiator;
        let scope = oc_client.context().scope.to_owned();

        let text = format!("user_id: {}\n\nscope: {:?}", user_id, scope);

        // Send the message to OpenChat but don't wait for the response
        let message = oc_client
            .send_text_message(text)
            .with_block_level_markdown(true)
            .execute_then_return_message(|args, response| match response {
                Ok(send_message::Response::Success(_)) => {}
                error => {
                    ic_cdk::println!("send_text_message: {args:?}, {error:?}");
                }
            });

        Ok(SuccessResult { message })
    }
}

impl Price {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "price".to_string(),
            description: Some(
                "This will return price of configured Cryptocurrency or FiatCurrency".to_string(),
            ),
            placeholder: Some("Getting latest price ...".to_string()),
            params: vec![],
            permissions: BotPermissions::text_only(),
            default_role: None,
            direct_messages: true,
        }
    }
}

/*
kongswap method:

kongswap api :https://github.com/KongSwap/kong/blob/main/src/kong_svelte/src/lib/api/index.ts

*/
