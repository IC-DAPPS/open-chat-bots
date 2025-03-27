use crate::price_provider::icpswap::{get_icrc_ledger_name, get_latest_price};
use crate::price_provider::{format_float, get_expiration_time};
use crate::stable::price_map::{self, PriceStore};
use async_trait::async_trait;
use candid::Principal;
use oc_bots_sdk::api::command::{CommandHandler, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::actions::send_message;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, ChatRole};
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

static DEFINITION: LazyLock<BotCommandDefinition> =
    LazyLock::new(ConfigICPSwapProvider::definition);

pub struct ConfigICPSwapProvider;

#[async_trait]
impl CommandHandler<CanisterRuntime> for ConfigICPSwapProvider {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        let canister_id_string = oc_client
            .context()
            .command
            .arg::<String>("Ledger_CanisterId");

        let canister_id = Principal::from_text(&canister_id_string)
            .map_err(|e| format!("Invalid canister ID: {}", e))?; // `?` operator will return the the maped error "Invalid canister ID: {}" if the fromText returns Err() . if Ok() we use the value // https://gist.github.com/ahdrahees/7e692d0df04d4df25aa6b2282aaf93e2

        let (ledger_name,) = get_icrc_ledger_name(canister_id)
            .await
            .map_err(|e| format!("Failed to get ledger name: {:?}", e))?;

        let price = get_latest_price(canister_id).await?;

        let reply = format!(
            "Configured ICPSwap as provider for {ledger_name} \nCurrent Price of {ledger_name} ${}",
            format_float(price)
        );

        price_map::insert(
            canister_id.to_string(),
            PriceStore {
                price,
                expiration_time: get_expiration_time(),
                name: Some(ledger_name),
            },
        );

        // Send the message to OpenChat but don't wait for the response
        let message = oc_client
            .send_text_message(reply)
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

impl ConfigICPSwapProvider {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "configure_bot_price_provider_icpswap".to_string(),
            description: Some("Use this command to configure price bot using ICPSwap".to_string()),
            placeholder: Some("Configuring ...".to_string()),
            params: vec![BotCommandParam {
                name: "Ledger_CanisterId".to_string(),
                description: Some("ICRC Ledger Canister Id".to_string()),
                placeholder: Some("Enter canister ID".to_string()),
                required: true,
                param_type: BotCommandParamType::StringParam(StringParam {
                    min_length: 26,
                    max_length: 28,
                    choices: Vec::new(),
                    multi_line: false,
                }),
            }],
            permissions: BotPermissions::text_only(),
            default_role: Some(ChatRole::Admin),
            direct_messages: false,
        }
    }
}
