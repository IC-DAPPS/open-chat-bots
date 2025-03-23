use async_trait::async_trait;
use oc_bots_sdk::api::command::{CommandHandler, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::actions::send_message;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::BotCommandContext;
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

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
            "Configured for Price of {}/{}\n Base Class: {}\n Quote Class: {}",
            base_asset_symbol, quote_asset_symbol, base_asset_class, quote_asset_class
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

impl ConfigXRCProvider {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "configure_bot_price_provider_exchange_rate_canister".to_string(),
            description: Some(
                "Use this command to configure price bot using Exchange Rate Canister".to_string(),
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
            default_role: None,
            direct_messages: false,
        }
    }
}
