use async_trait::async_trait;
use oc_bots_sdk::api::command::{CommandHandler, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::BotCommandContext;
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

use crate::message::send_message_with_visibility_option;

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(Echo::definition);

pub struct Echo;

#[async_trait]
impl CommandHandler<CanisterRuntime> for Echo {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        let text: String = oc_client.context().command.arg("message");

        Ok(send_message_with_visibility_option(text, &oc_client))
    }
}

impl Echo {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "echo".to_string(),
            description: Some("This will echo any text".to_string()),
            placeholder: None,
            params: vec![
                BotCommandParam {
                    name: "message".to_string(),
                    description: Some("The message to echo".to_string()),
                    param_type: BotCommandParamType::StringParam(StringParam {
                        min_length: 1,
                        max_length: 1000,
                        choices: Vec::new(),
                        multi_line: true,
                    }),
                    required: true,
                    placeholder: None,
                },
                // BotCommandParam {
                //     name: "only_visible_to_me".to_string(),
                //     description: Some(
                //         "If true, the message will only be visible to the sender else visible to everyone".to_string(),
                //     ),
                //     param_type: BotCommandParamType::BooleanParam,
                //     required: false,
                //     placeholder: Some("Only visible to you".to_string()),
                // },
                BotCommandParam {
                    name: "Visibility".to_string(),
                    description: Some("The visibility of the message".to_string()),
                    param_type: BotCommandParamType::StringParam(StringParam {
                        min_length: 1,
                        max_length: 1000,
                        choices: vec![
                            BotCommandOptionChoice {
                                name: "Only to me".to_string(),
                                value: "Only to me".to_string(),
                            },
                            BotCommandOptionChoice {
                                name: "Everyone".to_string(),
                                value: "Everyone".to_string(),
                            },
                        ],
                        multi_line: false,
                    }),
                    required: true,
                    placeholder: Some("Message visibility".to_string()),
                },
            ],
            permissions: BotPermissions::text_only(),
            default_role: None,
            direct_messages: Some(false),
        }
    }
}
