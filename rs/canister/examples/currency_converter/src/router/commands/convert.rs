use crate::country::get_flag_currency_name_currency_code_as_bot_cmd_choices;
use crate::price_provider::xrc::{get_latest_price, Asset};
use async_trait::async_trait;
use iso_currency::Currency;
use oc_bots_sdk::api::command::{CommandHandler, EphemeralMessageBuilder, Message, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::actions::send_message;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, BotCommandScope, ChatRole, MessageContentInitial};
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(CurrencyConverter::definition);

pub struct CurrencyConverter;

#[async_trait]
impl CommandHandler<CanisterRuntime> for CurrencyConverter {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        let from = oc_client.context().command.arg::<String>("From");
        let to = oc_client.context().command.arg::<String>("To");

        let amount = oc_client.context().command.arg::<f64>("Amount");

        let scope = oc_client.context().scope.to_owned();

        match helper_function(scope, from, to, amount).await {
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

async fn helper_function(
    scope: BotCommandScope,
    from: String,
    to: String,
    amount: f64,
) -> Result<String, String> {
    let base_asset = Asset::new_from_strings("FiatCurrency", from.clone())?;
    let quote_asset = Asset::new_from_strings("FiatCurrency", to.clone())?;

    let price = get_latest_price(base_asset.clone(), quote_asset.clone()).await?;

    let from_currency =
        Currency::from_code(&from).expect("Valid currency code got from the bot choice params");
    let to_currency =
        Currency::from_code(&to).expect("Valid currency code got from the bot choice params");

    let converted_amount = price * amount;
    let reply = format!(
        "{} {} = **{} {}**",
        amount, from_currency, converted_amount, to_currency
    );
    Ok(reply)
}

impl CurrencyConverter {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "convert".to_string(),
            description: Some("Convert currency from one to another".to_string()),
            placeholder: None,
            params: vec![
                BotCommandParam {
                    name: "From".to_string(),
                    description: Some("From Currency".to_string()),
                    placeholder: None,
                    required: true,
                    param_type: BotCommandParamType::StringParam(StringParam {
                        min_length: 0,
                        max_length: 100,
                        choices: get_flag_currency_name_currency_code_as_bot_cmd_choices(),
                        multi_line: false,
                    }),
                },
                BotCommandParam {
                    name: "To".to_string(),
                    description: Some("To Currency".to_string()),
                    placeholder: None,
                    required: true,
                    param_type: BotCommandParamType::StringParam(StringParam {
                        min_length: 0,
                        max_length: 100,
                        choices: get_flag_currency_name_currency_code_as_bot_cmd_choices(),
                        multi_line: false,
                    }),
                },
                BotCommandParam {
                    name: "Amount".to_string(),
                    description: Some("Amount to convert".to_string()),
                    placeholder: None,
                    required: true,
                    param_type: BotCommandParamType::DecimalParam(DecimalParam {
                        min_value: 0.0,
                        max_value: f64::MAX,
                        choices: Vec::new(),
                    }),
                },
            ],
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

pub fn send_message(
    text: String,
    oc_client: &Client<CanisterRuntime, BotCommandContext>,
) -> Option<Message> {
    // Send the message to OpenChat but don't wait for the response
    oc_client
        .send_text_message(text)
        .with_block_level_markdown(true)
        .execute_then_return_message(|args, response| match response {
            Ok(send_message::Response::Success(_)) => {}
            error => {
                ic_cdk::println!("send_text_message: {args:?}, {error:?}");
            }
        })
}
