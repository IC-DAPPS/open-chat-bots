use async_trait::async_trait;
use oc_bots_sdk::api::command::{CommandHandler, EphemeralMessageBuilder, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, BotCommandScope, MessageContentInitial};
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(Help::definition);

pub struct Help;

#[async_trait]
impl CommandHandler<CanisterRuntime> for Help {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        // let user_id = oc_client.context().command.initiator;
        // let scope = oc_client.context().scope.to_owned();

        // let text = `format!("user_id: {}\n\nscope: {:?}", user_id, scope);

        let reply = "What is Price Bot?
        Price Bot is a bot that shows the price of a crypto/fiat currency. Each community or group can configure the bot to get the price of any crypto/fiat currency according to their needs.
        ---
        How to use the bot?

        - `/price` : Get the price of a crypto/fiat currency configured by the community as message only visible to you.
        - `/price_message` : Get the price of a crypto/fiat currency configured by the community as message visible to everyone in the group or community.
        - `/configure_bot_price_provider_exchange_rate_canister` : Configure the bot to get the price of a crypto/fiat currency from the Exchange Rate Canister. *Only community or group administrators can use this command*.
        - `/configure_bot_price_provider_icpswap` : Configure the bot to fetch prices for ICRC tokens in the ICP ecosystem that are listed on ICPSwap. *Only community or group administrators can use this command*.
        ---
        How to use the configure_bot_price_provider_exchange_rate_canister command?

        - You have to be an administrator of the community or group to use this command.
        - Type the command `/configure_bot_price_provider_exchange_rate_canister` in the group or community.
        - Popup will appear asking for the `Base_Asset_Symbol`,`Base_Asset_Class`, `Quote_Asset_Symbol` and `Quote_Asset_Class`.
        - For example if you are configureing for Bitcoin price fill input as `BTC` for `Base_Asset_Symbol`, select `Cryptocurrency` for `Base_Asset_Class`, `USD` for `Quote_Asset_Symbol` and `Fiat Currency` for `Quote_Asset_Class`.
        - If configured successfully, price bot will show the price of Bitcoin in USD in response message.
        ---
        How to use the configure_bot_price_provider_icpswap command ?

        - You have to be an administrator of the community or group to use this command.
        - Type the command `/configure_bot_price_provider_icpswap` in the group or community.
        - Popup will appear asking for the `Ledger_Canister_Id` Its the canister id of the ICRC Ledger.
        - For example if you are configureing for CHAT token price fill input as `2ouva-viaaa-aaaaq-aaamq-cai` for `Ledger_Canister_Id`.
        - If configured successfully, price bot will show the price of CHAT token in USD in response message.
        ".to_string();

        Ok(send_ephemeral_message(reply, &oc_client.context().scope))
    }
}

impl Help {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "/help".to_string(),
            description: Some("How to use the bot".to_string()),
            placeholder: None,
            params: vec![],
            permissions: BotPermissions::text_only(),
            default_role: None,
            direct_messages: false,
        }
    }
}

fn send_ephemeral_message(reply: String, scope: &BotCommandScope) -> SuccessResult {
    // Reply to the initiator with an ephemeral message
    EphemeralMessageBuilder::new(
        MessageContentInitial::from_text(reply),
        scope.message_id().unwrap(),
    )
    .build()
    .into()
}

/*
kongswap method:

kongswap api :https://github.com/KongSwap/kong/blob/main/src/kong_svelte/src/lib/api/index.ts

*/
