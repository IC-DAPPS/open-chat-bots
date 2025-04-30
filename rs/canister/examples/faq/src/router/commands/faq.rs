use crate::message::{send_ephemeral_message, send_message};
use crate::stable::faq_map::{self, Key};
use async_trait::async_trait;
use oc_bots_sdk::api::command::{CommandHandler, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, BotCommandScope};
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(FAQ::definition);

pub struct FAQ;

#[async_trait]
impl CommandHandler<CanisterRuntime> for FAQ {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        let scope = oc_client.context().scope.to_owned();

        match get_faq(scope) {
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

fn get_faq(scope: BotCommandScope) -> Result<String, String> {
    let key: Key = Key::from_bot_cmd_scope(scope)?;

    // Try to get FAQs for this chat/channel. If none exist, return a helpful message
    let faq = faq_map::get(&key).ok_or(
        "No FAQs have been set up yet. Please ask a Moderator to add some FAQs for this community or group.",
    )?;

    Ok(faq)
}

impl FAQ {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "FAQs".to_string(),
            description: Some("Frequently Asked Questions".to_string()),
            placeholder: None,
            params: vec![],
            permissions: BotPermissions::text_only(),
            default_role: None,
            direct_messages: Some(true),
        }
    }
}
