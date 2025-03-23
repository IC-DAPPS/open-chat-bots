use crate::state;
use configure_bot_price_provider_exchange_rate_canister::ConfigXRCProvider;
use configure_bot_price_provider_icpswap::ConfigICPSwapProvider;
use echo::Echo;
use oc_bots_sdk::api::command::CommandHandlerRegistry;
use oc_bots_sdk::api::definition::BotCommandDefinition;
use oc_bots_sdk_canister::env::now;
use oc_bots_sdk_canister::http_command_handler;
use oc_bots_sdk_canister::CanisterRuntime;
use oc_bots_sdk_canister::OPENCHAT_CLIENT_FACTORY;
use oc_bots_sdk_canister::{HttpRequest, HttpResponse};
use price::Price;
use std::sync::LazyLock;

mod configure_bot_price_provider_exchange_rate_canister;
mod configure_bot_price_provider_icpswap;
mod echo;
mod price;

static COMMANDS: LazyLock<CommandHandlerRegistry<CanisterRuntime>> = LazyLock::new(|| {
    CommandHandlerRegistry::new(OPENCHAT_CLIENT_FACTORY.clone())
        .register(Echo)
        .register(Price)
        .register(ConfigXRCProvider)
        .register(ConfigICPSwapProvider)
});

pub fn definitions() -> Vec<BotCommandDefinition> {
    COMMANDS.definitions()
}

pub async fn execute(request: HttpRequest) -> HttpResponse {
    let public_key = state::read(|state| state.oc_public_key().to_string());
    let now = now();

    http_command_handler::execute(request, &COMMANDS, &public_key, now).await
}
