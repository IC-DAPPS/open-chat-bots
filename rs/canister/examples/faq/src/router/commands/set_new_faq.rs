use async_trait::async_trait;
use oc_bots_sdk::api::command::{CommandHandler, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, BotCommandScope, ChatRole};
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

use crate::message::send_ephemeral_message;
use crate::stable::faq_map::{self, Key};

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(SetNewFAQ::definition);

pub struct SetNewFAQ;

#[async_trait]
impl CommandHandler<CanisterRuntime> for SetNewFAQ {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        let faq_message = oc_client.context().command.arg::<String>("FAQ_Message");
        let scope = oc_client.context().scope.to_owned();

        match store_faq(faq_message, scope) {
            Ok(reply) => Ok(send_ephemeral_message(reply, &oc_client.context().scope)),
            Err(err_message) => Ok(send_ephemeral_message(
                err_message,
                &oc_client.context().scope,
            )),
        }
    }
}

fn store_faq(faq_message: String, scope: BotCommandScope) -> Result<String, String> {
    let key: Key = Key::from_bot_cmd_scope(scope)?;

    faq_map::insert(key, faq_message);

    Ok("New FAQ set. Use /FAQs command to check it.".to_string())
}

impl SetNewFAQ {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "set_new_faq".to_string(),
            description: Some("Use this command to set a new FAQ".to_string()),
            placeholder: Some("Setting new FAQs...".to_string()),
            params: vec![BotCommandParam {
                name: "FAQ_Message".to_string(),
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
