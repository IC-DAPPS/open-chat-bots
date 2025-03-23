use async_trait::async_trait;
use oc_bots_sdk::api::command::{CommandHandler, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::actions::send_message;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, ChatRole};
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(ConfigureBot::definition);

pub struct ConfigureBot;

#[async_trait]
impl CommandHandler<CanisterRuntime> for ConfigureBot {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        let text = oc_client.context().command.arg("Price_Provider");

        let reply = "Configured".to_string();

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

// pub struct StringParam {
//     pub min_length: u16,
//     pub max_length: u16,
//     pub choices: Vec<BotCommandOptionChoice<String>>,
//     #[serde(default)]
//     pub multi_line: bool,
// }

impl ConfigureBot {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "configure_bot".to_string(),
            description: Some(
                "Use this command to configure price bot according to your need".to_string(),
            ),
            placeholder: Some("Configuring ...".to_string()),
            params: vec![
                BotCommandParam {
                    name: "Price_Provider".to_string(),
                    description: Some(
                        "Price provider to fetch the price of Crypto or Fiat".to_string(),
                    ),
                    placeholder: Some("Select Price Provider".to_string()),
                    required: true,
                    param_type: BotCommandParamType::StringParam(StringParam {
                        min_length: 1,
                        max_length: 50,
                        choices: vec![
                            BotCommandOptionChoice {
                                name: "Exchange Rate Canister".to_string(),
                                value: "xrc".to_string(),
                            },
                            BotCommandOptionChoice {
                                name: "ICPSwap".to_string(),
                                value: "icpswap".to_string(),
                            },
                        ],
                        multi_line: false,
                    }),
                },
                BotCommandParam {
                    name: "Base_Asset_Symbol_for_Exchange_Rate_Canister".to_string(),
                    description: Some(
                        "Provide Base Asset Symbol if Exchange Rate Canister is selected as price provider"
                            .to_string(),
                    ),
                    placeholder: Some(
                        "Enter if Exchange Rate Canister is selected as provider".to_string(),
                    ),
                    required: false,
                    param_type: BotCommandParamType::StringParam(StringParam {
                        min_length: 2,
                        max_length: 10,
                        choices: Vec::new(),
                        multi_line: false,
                    }),
                },
                BotCommandParam {
                    name: "Base_Asset_Class_for_Exchange_Rate_Canister".to_string(),
                    description: Some(
                        "Provide Base Asset Class if Exchange Rate Canister is selected as price provider"
                            .to_string(),
                    ),
                    placeholder: Some(
                        "Enter if Exchange Rate Canister is selected as provider".to_string(),
                    ),
                    required: false,
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
                    name: "Quote_Asset_Symbol_for_Exchange_Rate_Canister".to_string(),
                    description: Some(
                        "Provide Quote Asset Symbol if Exchange Rate Canister is selected as price provider. If you are using for fetching price of Crypto input \"USDT\" or \"USD\" as Quote Asset"
                            .to_string(),
                    ),
                    placeholder: Some(
                        "Enter if Exchange Rate Canister is selected as provider".to_string(),
                    ),
                    required: false,
                    param_type: BotCommandParamType::StringParam(StringParam {
                        min_length: 2,
                        max_length: 10,
                        choices: Vec::new(),
                        multi_line: false,
                    }),
                },

                BotCommandParam {
                    name: "Quote_Asset_Class_for_Exchange_Rate_Canister".to_string(),
                    description: Some(
                        "Provide Quote Asset Class if Exchange Rate Canister is selected as price provider"
                            .to_string(),
                    ),
                    placeholder: Some(
                        "Enter if Exchange Rate Canister is selected as provider".to_string(),
                    ),
                    required: false,
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
                    name: "Testing".to_string(),
                    description: Some(
                        "Provide Base Asset Symbol if Exchange Rate Canister is selected as price provider"
                            .to_string(),
                    ),
                    placeholder: Some(
                        "Enter if Exchange Rate Canister is selected as provider".to_string(),
                    ),
                    required: true,
                    param_type: BotCommandParamType::StringParam(StringParam {
                        min_length: 2,
                        max_length: 10,
                        choices: Vec::new(),
                        multi_line: false,
                    }),
                },
            ],
            permissions: BotPermissions::text_only(),
            default_role: Some(ChatRole::Admin),
            direct_messages: false,
        }
    }
}
