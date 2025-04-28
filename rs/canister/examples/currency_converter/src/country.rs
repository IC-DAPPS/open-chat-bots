use country_emoji::code_to_flag;
use iso_currency::Currency;
use iso_currency::IntoEnumIterator;
use oc_bots_sdk::api::definition::BotCommandOptionChoice;

pub fn get_flag_currency_name_currency_code_as_bot_cmd_choices(
) -> Vec<BotCommandOptionChoice<String>> {
    let all_currencies_iterator = Currency::iter();

    all_currencies_iterator
        .map(|currency| {
            let country_emoji = code_to_flag(currency.code()).unwrap_or_default();

            BotCommandOptionChoice {
                name: format!("{} {} {}", country_emoji, currency.code(), currency.name()),
                value: currency.code().to_string(),
            }
        })
        .collect()
}
