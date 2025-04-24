use crate::price_provider::{format_float, icpswap, xrc};
use crate::stable::config_map::{self, Config, ConfigKey};
use crate::stable::price_map::{self, price_key_from_config, PriceStore};
use async_trait::async_trait;
use ic_cdk::api::time;
use oc_bots_sdk::api::command::{CommandHandler, EphemeralMessageBuilder, Message, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::actions::send_message;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, BotCommandScope, MessageContentInitial};
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(PriceMessage::definition);

pub struct PriceMessage;

#[async_trait]
impl CommandHandler<CanisterRuntime> for PriceMessage {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        // let user_id = oc_client.context().command.initiator;
        let scope = oc_client.context().scope.to_owned();

        // let text = format!("user_id: {}\n\nscope: {:?}", user_id, scope);

        match get_price_message(scope).await {
            Ok(reply) => Ok(SuccessResult {
                message: send_message(reply, &oc_client),
            }),
            Err(err_message) => Ok(send_ephemeral_message(
                err_message,
                &oc_client.context().scope,
            )),
        }
    }
}

impl PriceMessage {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "price_message".to_string(),
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

async fn get_price_message(scope: BotCommandScope) -> Result<String, String> {
    let config_key = ConfigKey::from_bot_cmd_scope(scope);
    let config = config_map::get(config_key)
        .ok_or("Price config not found. Admin or Owner can set new price config.")?;

    let price_key = price_key_from_config(&config);

    let price_store = price_map::get(&price_key).ok_or("PriceMessage not exist in map")?;

    if time() < price_store.expiration_time {
        let message = match &price_store.name {
            Some(name) => format!(
                "Current PriceMessage of {name} is ${}",
                format_float(price_store.price)
            ), // Name is not none for ICPSwap
            None => {
                let (base, quote) = config
                    .xrc_asset_symbols()
                    .ok_or("Failed to get base and quote symbols")?;

                format!(
                    "Current PriceMessage of {base} is {} {quote}",
                    format_float(price_store.price)
                )
            } // Name field none for XRC.
        };

        Ok(message)
    } else {
        let (price, expiration_time) = match config.clone() {
            Config::ICPSwap { canister_id } => icpswap::get_latest_price(canister_id).await?,
            Config::XRC {
                base_asset,
                quote_asset,
            } => xrc::get_latest_price(base_asset, quote_asset).await?,
        };

        let message = match &price_store.name {
            Some(name) => format!("Current PriceMessage of {name} is ${}", format_float(price)), // Name is not none for ICPSwap
            None => {
                let (base, quote) = config
                    .xrc_asset_symbols()
                    .ok_or("Failed to get base and quote symbols")?;

                format!("Current Price of {base} is {} {quote}", format_float(price))
            } // Name field none for XRC.
        };

        price_map::insert(
            price_key,
            PriceStore {
                price,
                expiration_time,
                name: price_store.name,
            },
        );

        Ok(message)
    }
}

fn send_message(
    text: String,
    oc_client: &Client<CanisterRuntime, BotCommandContext>,
) -> Option<Message> {
    // Send the message to OpenChat but don't wait for the response
    oc_client
        .send_text_message(text)
        .with_block_level_markdown(true)
        .execute_then_return_message(|args, response| match response {
            Ok(send_message::Response::Success(_)) => {}
            error => {
                ic_cdk::println!("send_text_message: {args:?}, {error:?}");
            }
        })
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

/*
kongswap method:

kongswap api :https://github.com/KongSwap/kong/blob/main/src/kong_svelte/src/lib/api/index.ts

*/
