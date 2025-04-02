use crate::price_provider::format_float;
use crate::price_provider::icpswap::{get_icrc_ledger_name, get_latest_price};
use crate::stable::config_map::{self, Config, ConfigKey};
use crate::stable::price_map::{self, PriceStore};
use async_trait::async_trait;
use candid::Principal;
use oc_bots_sdk::api::command::{CommandHandler, EphemeralMessageBuilder, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, BotCommandScope, ChatRole, MessageContentInitial};
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
        let scope = oc_client.context().scope.to_owned();

        match helper_function(scope, &canister_id_string).await {
            Ok(reply) => Ok(send_ephemeral_message(reply, &oc_client.context().scope)),
            Err(err_message) => Ok(send_ephemeral_message(
                err_message,
                &oc_client.context().scope,
            )),
        }
    }
}

async fn helper_function(
    scope: BotCommandScope,
    canister_id_string: &String,
) -> Result<String, String> {
    let canister_id = Principal::from_text(&canister_id_string)
        .map_err(|e| format!("Invalid canister ID: {}", e))?; // `?` operator will return the the maped error "Invalid canister ID: {}" if the fromText returns Err() . if Ok() we use the value // https://gist.github.com/ahdrahees/7e692d0df04d4df25aa6b2282aaf93e2

    let (ledger_name,) = get_icrc_ledger_name(canister_id)
        .await
        .map_err(|e| format!("Failed to get ledger name: {:?}", e))?;

    let (price, expiration_time) = get_latest_price(canister_id).await?;

    let reply = format!(
        "Configured ICPSwap as provider for {ledger_name} \nCurrent Price of {ledger_name} is ${}",
        format_float(price)
    );

    let config_key = ConfigKey::from_bot_cmd_scope(scope);
    config_map::insert(config_key, Config::ICPSwap { canister_id });

    price_map::insert(
        canister_id.to_string(),
        PriceStore {
            price,
            expiration_time,
            name: Some(ledger_name),
        },
    );

    Ok(reply)
}

impl ConfigICPSwapProvider {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "configure_bot_price_provider_icpswap".to_string(),
            description: Some("Use this command to configure price bot using ICPSwap.  It returns an Ephemeral message that will only be visible for the user that initiated interaction with a bot, and it will disappear upon UI refresh.".to_string()),
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

fn send_ephemeral_message(reply: String, scope: &BotCommandScope) -> SuccessResult {
    // Reply to the initiator with an ephemeral message
    EphemeralMessageBuilder::new(
        MessageContentInitial::from_text(reply),
        scope.message_id().unwrap(),
    )
    .build()
    .into()
}
