use crate::message::send_ephemeral_message;
use crate::stable::faq_map::{self, Key};
use async_trait::async_trait;
use oc_bots_sdk::api::command::{CommandHandler, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, BotCommandScope};
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(DeleteFAQ::definition);

pub struct DeleteFAQ;

#[async_trait]
impl CommandHandler<CanisterRuntime> for DeleteFAQ {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        let scope = oc_client.context().scope.to_owned();

        match delete_faq(scope) {
            Ok(reply) => Ok(send_ephemeral_message(reply, &oc_client.context().scope)),
            Err(err_message) => Ok(send_ephemeral_message(
                err_message,
                &oc_client.context().scope,
            )),
        }
    }
}

fn delete_faq(scope: BotCommandScope) -> Result<String, String> {
    let key: Key = Key::from_bot_cmd_scope(scope)?;

    faq_map::remove(key).ok_or("No FAQs to delete.")?;

    Ok("Deleted FAQs. Use /FAQs command to check it.".to_string())
}

impl DeleteFAQ {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "delete_faq".to_string(),
            description: Some("Delete FAQs".to_string()),
            placeholder: Some("Deleting FAQs...".to_string()),
            params: vec![],
            permissions: BotPermissions::text_only(),
            default_role: None,
            direct_messages: Some(true),
        }
    }
}
