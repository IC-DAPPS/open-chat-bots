use crate::country::get_flag_currency_name_currency_code_as_bot_cmd_choices;
use crate::crypto::{get_crypto_currency_choices, get_crypto_symbols};
use crate::price_provider::xrc::{get_latest_price, Asset, AssetClass};
use async_trait::async_trait;
use iso_currency::Currency;
use oc_bots_sdk::api::command::{CommandHandler, EphemeralMessageBuilder, Message, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::actions::send_message;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, BotCommandScope, MessageContentInitial};
use oc_bots_sdk_canister::CanisterRuntime;
use std::sync::LazyLock;
use std::collections::HashSet;

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(CurrencyConverter::definition);
static CRYPTO_SYMBOLS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    get_crypto_symbols().into_iter().collect()
});

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
        // Get from and to currency codes
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

// Helper function to determine if a currency code is a cryptocurrency
fn is_crypto_currency(symbol: &str) -> bool {
    CRYPTO_SYMBOLS.contains(symbol)
}

async fn helper_function(
    _scope: BotCommandScope,
    from: String,
    to: String,
    amount: f64,
) -> Result<String, String> {
    // Determine asset classes based on the currency codes
    let from_asset_class = if is_crypto_currency(&from) {
        AssetClass::Cryptocurrency
    } else {
        AssetClass::FiatCurrency
    };
    
    let to_asset_class = if is_crypto_currency(&to) {
        AssetClass::Cryptocurrency
    } else {
        AssetClass::FiatCurrency
    };

    // Create assets with the appropriate types
    let base_asset = Asset::new(from_asset_class.clone(), from.clone());
    let quote_asset = Asset::new(to_asset_class.clone(), to.clone());

    // Get exchange rate
    let price = get_latest_price(base_asset.clone(), quote_asset.clone()).await?;

    // Format the result based on currency types
    let converted_amount = price * amount;
    
    // Format the response differently for crypto and fiat
    let reply = match (from_asset_class, to_asset_class) {
        (AssetClass::FiatCurrency, AssetClass::FiatCurrency) => {
            // Fiat to Fiat - use Currency for formatting
            let from_currency = Currency::from_code(&from)
                .ok_or_else(|| format!("Invalid fiat currency code: {}", from))?;
            let to_currency = Currency::from_code(&to)
                .ok_or_else(|| format!("Invalid fiat currency code: {}", to))?;
            
            format!(
                "{} {} = **{} {}**",
                amount, from_currency, converted_amount, to_currency
            )
        },
        (AssetClass::Cryptocurrency, AssetClass::Cryptocurrency) => {
            // Crypto to Crypto
            format!(
                "{} {} = **{} {}**",
                amount, from, format_crypto_amount(converted_amount), to
            )
        },
        (AssetClass::FiatCurrency, AssetClass::Cryptocurrency) => {
            // Fiat to Crypto
            let from_currency = Currency::from_code(&from)
                .ok_or_else(|| format!("Invalid fiat currency code: {}", from))?;
            
            format!(
                "{} {} = **{} {}**",
                amount, from_currency, format_crypto_amount(converted_amount), to
            )
        },
        (AssetClass::Cryptocurrency, AssetClass::FiatCurrency) => {
            // Crypto to Fiat
            let to_currency = Currency::from_code(&to)
                .ok_or_else(|| format!("Invalid fiat currency code: {}", to))?;
            
            format!(
                "{} {} = **{} {}**",
                amount, from, converted_amount, to_currency
            )
        },
    };
    
    Ok(reply)
}

// Format crypto amounts with appropriate decimal places
fn format_crypto_amount(amount: f64) -> String {
    if amount >= 1.0 {
        format!("{:.2}", amount)
    } else if amount >= 0.01 {
        format!("{:.4}", amount)
    } else if amount >= 0.0001 {
        format!("{:.6}", amount)
    } else {
        format!("{:.8}", amount)
    }
}

impl CurrencyConverter {
    fn definition() -> BotCommandDefinition {
        // Get both fiat and crypto currencies for the dropdown
        let fiat_choices = get_flag_currency_name_currency_code_as_bot_cmd_choices();
        let crypto_choices = get_crypto_currency_choices();
        
        // Combine both lists for the dropdowns
        let mut all_choices = fiat_choices.clone();
        all_choices.extend(crypto_choices.clone());

        BotCommandDefinition {
            name: "convert".to_string(),
            description: Some("Convert between fiat currencies and cryptocurrencies".to_string()),
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
                        choices: all_choices.clone(), // Use combined list of currencies
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
                        choices: all_choices, // Use combined list of currencies 
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
            direct_messages: true,
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
