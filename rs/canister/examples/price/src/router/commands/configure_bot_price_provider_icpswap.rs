use async_trait::async_trait;
use oc_bots_sdk::api::command::{CommandHandler, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::actions::send_message;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, ChatRole};
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

static DEFINITION: LazyLock<BotCommandDefinition> =
    LazyLock::new(ConfigICPSwapProvider::definition);

pub struct ConfigICPSwapProvider;

#[async_trait]
impl CommandHandler<CanisterRuntime> for ConfigICPSwapProvider {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        // let user_id = oc_client.context().command.initiator;
        // let scope = oc_client.context().scope.to_owned();

        let canister_id = oc_client
            .context()
            .command
            .arg::<String>("Ledger_CanisterId");

        let reply = format!(
            "Configured for ICPSwap provider with canister ID: {}",
            canister_id
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

impl ConfigICPSwapProvider {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "configure_bot_price_provider_icpswap".to_string(),
            description: Some("Use this command to configure price bot using ICPSwap".to_string()),
            placeholder: Some("Configuring ...".to_string()),
            params: vec![BotCommandParam {
                name: "Ledger_CanisterId".to_string(),
                description: Some("ICRC Ledger Canister Id".to_string()),
                placeholder: Some("Enter canister ID".to_string()),
                required: true,
                param_type: BotCommandParamType::StringParam(StringParam {
                    min_length: 26,
                    max_length: 28,
                    choices: Vec::new(),
                    multi_line: false,
                }),
            }],
            permissions: BotPermissions::text_only(),
            default_role: Some(ChatRole::Admin),
            direct_messages: false,
        }
    }
}
