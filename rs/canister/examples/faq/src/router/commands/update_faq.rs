use async_trait::async_trait;
use oc_bots_sdk::api::command::{CommandHandler, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, BotCommandScope, ChatRole};
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

use crate::message::send_ephemeral_message;
use crate::stable::faq_map::{self, Key};

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(UpdateFAQ::definition);

pub struct UpdateFAQ;

#[async_trait]
impl CommandHandler<CanisterRuntime> for UpdateFAQ {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        let faq_message = oc_client.context().command.arg::<String>("Add");
        let scope = oc_client.context().scope.to_owned();

        match add_faq_to_existing_faqs(faq_message, scope) {
            Ok(reply) => Ok(send_ephemeral_message(reply, &oc_client.context().scope)),
            Err(err_message) => Ok(send_ephemeral_message(
                err_message,
                &oc_client.context().scope,
            )),
        }
    }
}

fn add_faq_to_existing_faqs(faq_message: String, scope: BotCommandScope) -> Result<String, String> {
    if faq_message.is_empty() {
        return Err("FAQ message cannot be empty".to_string());
    }

    let key: Key = Key::from_bot_cmd_scope(scope)?;

    let mut faq: String = faq_map::get(&key).unwrap_or_default();

    // Add a new line if the FAQ is not empty and the message does not start with a new line
    if !faq.is_empty() && !faq_message.starts_with('\n') {
        faq.push('\n');
    }

    // Add the new FAQ message to the existing FAQ
    faq.push_str(&faq_message);

    faq_map::insert(key, faq);

    Ok("Updated FAQs. Use /FAQs command to check it.".to_string())
}

impl UpdateFAQ {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "update_faq".to_string(),
            description: Some(
                "Use this command to add question and answer to an existing FAQ".to_string(),
            ),
            placeholder: Some("Updating FAQs...".to_string()),
            params: vec![BotCommandParam {
                name: "Add".to_string(),
                description: Some("FAQ question and answer".to_string()),
                placeholder: None,
                required: true,
                param_type: BotCommandParamType::StringParam(StringParam {
                    min_length: 0,
                    max_length: 65_535,
                    choices: Vec::new(),
                    multi_line: true,
                }),
            }],
            permissions: BotPermissions::text_only(),
            default_role: Some(ChatRole::Moderator),
            direct_messages: Some(false),
        }
    }
}
