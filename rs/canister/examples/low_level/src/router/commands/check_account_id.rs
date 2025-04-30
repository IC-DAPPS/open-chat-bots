use crate::message::{send_ephemeral_message, send_message_with_visibility_option};
use async_trait::async_trait;
use oc_bots_sdk::api::command::{CommandHandler, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::BotCommandContext;
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(CheckAccountId::definition);

pub struct CheckAccountId;

#[async_trait]
impl CommandHandler<CanisterRuntime> for CheckAccountId {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        let accountid: String = oc_client.context().command.arg("AccountID");

        if is_valid_account_id(&accountid) {
            Ok(send_message_with_visibility_option(
                format!("{} is Valid", accountid),
                &oc_client,
            ))
        } else {
            Ok(send_ephemeral_message(
                format!("{accountid} is invalid"),
                &oc_client.context().scope,
            ))
        }
    }
}

impl CheckAccountId {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "check_accountid".to_string(),
            description: Some("Checks if the provided input is a valid Account ID".to_string()),
            placeholder: None,
            params: vec![
                BotCommandParam {
                    name: "AccountID".to_string(),
                    description: Some("The Account ID to check".to_string()),
                    param_type: BotCommandParamType::StringParam(StringParam {
                        min_length: 1,
                        max_length: 100,
                        choices: vec![],
                        multi_line: false,
                    }),
                    required: true,
                    placeholder: Some("Enter a Account ID".to_string()),
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

pub fn is_valid_account_id(s: &str) -> bool {
    // Optional: Check for specific length (64 characters)
    // If length doesn't matter, you can remove this check
    if s.len() != 64 {
        return false;
    }

    // Check if all characters are valid hex digits
    for c in s.chars() {
        if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')) {
            return false;
        }
    }

    true
}
