use async_trait::async_trait;
use oc_bots_sdk::api::command::{CommandHandler, EphemeralMessageBuilder, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, ChatRole, MessageContentInitial};
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

use crate::price_provider::xrc::{get_latest_price, Asset};
use crate::stable::config_map::{self, Config, ConfigKey};
use crate::stable::price_map::{self, price_key_from_base_quote_asset, PriceStore};

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(ConfigXRCProvider::definition);

pub struct ConfigXRCProvider;

#[async_trait]
impl CommandHandler<CanisterRuntime> for ConfigXRCProvider {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        let base_asset_symbol = oc_client
            .context()
            .command
            .arg::<String>("Base_Asset_Symbol");
        let quote_asset_symbol = oc_client
            .context()
            .command
            .arg::<String>("Quote_Asset_Symbol");

        let base_asset_class = oc_client
            .context()
            .command
            .arg::<String>("Base_Asset_Class");
        let quote_asset_class = oc_client
            .context()
            .command
            .arg::<String>("Quote_Asset_Class");

        let reply = format!(
            "Configured Exchange Rate Canister as provider for Price of {}/{}\n Base Class: {}\n Quote Class: {}\nCurrent rate of {base_asset_symbol}/{quote_asset_symbol} is ",
            base_asset_symbol, quote_asset_symbol, base_asset_class, quote_asset_class
        );

        let base_asset = Asset::new_from_strings(&base_asset_class, base_asset_symbol)?;
        let quote_asset = Asset::new_from_strings(&quote_asset_class, quote_asset_symbol)?;

        let (price, expiration_time) =
            get_latest_price(base_asset.clone(), quote_asset.clone()).await?;

        let price_key = price_key_from_base_quote_asset(&base_asset, &quote_asset);

        let scope = oc_client.context().scope.to_owned();
        let config_key = ConfigKey::from_bot_cmd_scope(scope);
        config_map::insert(
            config_key,
            Config::XRC {
                base_asset,
                quote_asset,
            },
        );

        price_map::insert(
            price_key,
            PriceStore {
                price,
                expiration_time,
                name: None,
            },
        );

        let reply = format!("{reply}{price}");

        // Reply to the initiator with an ephemeral message
        Ok(EphemeralMessageBuilder::new(
            MessageContentInitial::from_text(reply),
            oc_client.context().scope.message_id().unwrap(),
        )
        .build()
        .into())
    }
}

impl ConfigXRCProvider {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "configure_bot_price_provider_exchange_rate_canister".to_string(),
            description: Some(
                "Use this command to configure price bot using Exchange Rate Canister.  It returns an Ephemeral message that will only be visible for the user that initiated interaction with a bot, and it will disappear upon UI refresh.".to_string(),
            ),
            placeholder: Some("Configuring ...".to_string()),
            params: vec![BotCommandParam {
                name: "Base_Asset_Symbol".to_string(),
                description: Some(
                    "Base Asset is the asset you're pricing. It's the first currency in a currency pair.".to_string(),
                ),
                placeholder: Some(
                    "Enter Symbol (e.g., BTC, EUR, ETH or etc.)".to_string(),
                ),
                required: true,
                param_type: BotCommandParamType::StringParam(StringParam {
                    min_length: 2,
                    max_length: 10,
                    choices: Vec::new(),
                    multi_line: false,
                }),
            },
            BotCommandParam {
                name: "Base_Asset_Class".to_string(),
                description: Some(
                    "Class of the base asset."
                        .to_string(),
                ),
                placeholder: Some(
                    "Select Currency Class".to_string(),
                ),
                required: true,
                param_type: BotCommandParamType::StringParam(StringParam {
                    min_length: 2,
                    max_length: 20,
                    choices: vec![
                        BotCommandOptionChoice {
                            name: "Cryptocurrency".to_string(),
                            value: "Cryptocurrency".to_string(),
                        },
                        BotCommandOptionChoice {
                            name: "Fiat Currency".to_string(),
                            value: "FiatCurrency".to_string(),
                        },
                    ],
                    multi_line: false,
                }),
            },
            BotCommandParam {
                name: "Quote_Asset_Symbol".to_string(),
                description: Some(
                    "Quote Asset is the asset that denominates the price. It's the second currency in a currency pair. If you are using for fetching price of Crypto input \"USDT\" or \"USD\" as Quote Asset"
                        .to_string(),
                ),
                placeholder: Some(
                    "Enter Symbol (e.g., USDT, USD, or etc.)".to_string(),
                ),
                required: true,
                param_type: BotCommandParamType::StringParam(StringParam {
                    min_length: 2,
                    max_length: 10,
                    choices: Vec::new(),
                    multi_line: false,
                }),
            },

            BotCommandParam {
                name: "Quote_Asset_Class".to_string(),
                description: Some(
                    "Class of the quote asset."
                        .to_string(),
                ),
                placeholder: Some(
                    "Enter".to_string(),
                ),
                required: true,
                param_type: BotCommandParamType::StringParam(StringParam {
                    min_length: 2,
                    max_length: 20,
                    choices: vec![
                        BotCommandOptionChoice {
                            name: "Cryptocurrency".to_string(),
                            value: "Cryptocurrency".to_string(),
                        },
                        BotCommandOptionChoice {
                            name: "Fiat Currency".to_string(),
                            value: "FiatCurrency".to_string(),
                        },
                    ],
                    multi_line: false,
                }),
            },],
            permissions: BotPermissions::text_only(),
            default_role:  Some(ChatRole::Admin),
            direct_messages: false,
        }
    }
}
