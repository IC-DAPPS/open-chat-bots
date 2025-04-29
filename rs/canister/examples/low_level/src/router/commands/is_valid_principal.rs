use crate::message::{send_ephemeral_message, send_message_with_visibility_option};
use async_trait::async_trait;
use candid::Principal;
use oc_bots_sdk::api::command::{CommandHandler, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::BotCommandContext;
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(CheckPrincipal::definition);

pub struct CheckPrincipal;

#[async_trait]
impl CommandHandler<CanisterRuntime> for CheckPrincipal {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        let principal_str: String = oc_client.context().command.arg("Principal");

        match Principal::from_text(&principal_str) {
            Ok(principal) => Ok(send_message_with_visibility_option(
                format!("{} is Valid", principal),
                &oc_client,
            )),
            Err(err) => Ok(send_ephemeral_message(
                format!("{principal_str} is invalid, {}", err.to_string()),
                &oc_client.context().scope,
            )),
        }
    }
}

impl CheckPrincipal {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "check_principal".to_string(),
            description: Some(
                "Checks if the provided input is a valid principal identifier".to_string(),
            ),
            placeholder: None,
            params: vec![
                BotCommandParam {
                    name: "Principal".to_string(),
                    description: Some("The principal ID to check".to_string()),
                    param_type: BotCommandParamType::UserParam,
                    required: true,
                    placeholder: Some("Enter a Principal ID".to_string()),
                },
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
