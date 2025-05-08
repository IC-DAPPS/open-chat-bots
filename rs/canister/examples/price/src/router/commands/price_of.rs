use async_trait::async_trait;
use ic_cdk::api::time;
use oc_bots_sdk::api::command::{CommandHandler, EphemeralMessageBuilder, SuccessResult};
use oc_bots_sdk::api::definition::*;
use oc_bots_sdk::oc_api::actions::send_message;
use oc_bots_sdk::oc_api::client::Client;
use oc_bots_sdk::types::{BotCommandContext, BotCommandScope, MessageContentInitial};
use oc_bots_sdk_canister::CanisterRuntime;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::LazyLock;

use crate::price_provider::xrc::{Asset, AssetClass};
use crate::price_provider::{format_float, icpswap, xrc};
use crate::stable::config_map::{self, Config, ConfigKey};
use crate::stable::price_map::{self, price_key_from_config, PriceStore};

static DEFINITION: LazyLock<BotCommandDefinition> = LazyLock::new(PriceOf::definition);

pub struct PriceOf;

thread_local! {
    static TOKENS_MAP : RefCell<HashMap<String, Config>> = RefCell::new(get_tokens_map());
}

#[async_trait]
impl CommandHandler<CanisterRuntime> for PriceOf {
    fn definition(&self) -> &BotCommandDefinition {
        &DEFINITION
    }

    async fn execute(
        &self,
        oc_client: Client<CanisterRuntime, BotCommandContext>,
    ) -> Result<SuccessResult, String> {
        // let user_id = oc_client.context().command.initiator;
        let scope = oc_client.context().scope.to_owned();

        let token_select = oc_client.context().command.arg::<String>("Select");

        // let text = format!("user_id: {}\n\nscope: {:?}", user_id, scope);

        match get_price_message(scope, token_select).await {
            Ok(reply) => Ok(send_message(reply, &oc_client)),
            Err(err_message) => Ok(send_ephemeral_message(
                err_message,
                &oc_client.context().scope,
            )),
        }
    }
}

impl PriceOf {
    fn definition() -> BotCommandDefinition {
        BotCommandDefinition {
            name: "price_of".to_string(),
            description: Some("Get the price of Token in USD".to_string()),
            placeholder: Some("Getting latest price ...".to_string()),
            params: vec![BotCommandParam {
                name: "Select".to_string(),
                description: Some("Select a token to get its price".to_string()),
                placeholder: Some("Token".to_string()),
                required: true,
                param_type: BotCommandParamType::StringParam(StringParam {
                    min_length: 0,
                    max_length: 100,
                    choices: get_token_list(),
                    multi_line: false,
                }),
            }],
            permissions: BotPermissions::text_only(),
            default_role: None,
            direct_messages: true,
        }
    }
}

async fn get_price_message(scope: BotCommandScope, token: String) -> Result<String, String> {
    let token_config = TOKENS_MAP
        .with(|map| map.borrow().get(&token).cloned())
        .ok_or_else(|| "Token not found in the available token list")?;

    let price_key = price_key_from_config(&token_config);

    let price_store_opt = price_map::get(&price_key);

    match price_store_opt {
        Some(price_store) => {
            if time() < price_store.expiration_time {
                let message = get_price_message_from_price_store(&price_store, &token);
                Ok(message)
            } else {
                let message = get_price_message_from_fresh_price(
                    &token_config,
                    price_key,
                    price_store,
                    &token,
                )
                .await?;
                Ok(message)
            }
        }

        None => {
            let message = get_price_message_from_fresh_price(
                &token_config,
                price_key,
                PriceStore {
                    price: 0f64,
                    expiration_time: 0,
                    name: None, // instead of Some(token), here None chosen because here we going to use token variable.
                                // This will not affect communities who relay on Name (from price_map) for price message. because we will only
                                // reach this part of code for token that are not configured in any community. If a community configure this
                                // token in future, PriceStore.name will be updated by configuration function
                },
                &token,
            )
            .await?;
            Ok(message)
        }
    }
}

async fn get_price_message_from_fresh_price(
    token_config: &Config,
    price_key: String,
    price_store: PriceStore,
    token: &String,
) -> Result<String, String> {
    let (price, expiration_time) = match token_config.clone() {
        Config::ICPSwap { canister_id } => icpswap::get_latest_price(canister_id).await?,
        Config::XRC {
            base_asset,
            quote_asset,
        } => xrc::get_latest_price(base_asset, quote_asset).await?,
    };

    price_map::insert(
        price_key,
        PriceStore {
            price,
            expiration_time,
            name: price_store.name,
        },
    );

    let message = format!("Current Price of {token} is ${}", format_float(price));
    Ok(message)
}

fn get_price_message_from_price_store(price_store: &PriceStore, token: &String) -> String {
    format!(
        "Current Price of {token} is ${}",
        format_float(price_store.price)
    )
}

fn send_message(
    text: String,
    oc_client: &Client<CanisterRuntime, BotCommandContext>,
) -> SuccessResult {
    // Send the message to OpenChat but don't wait for the response
    SuccessResult {
        message: oc_client
            .send_text_message(text)
            .with_block_level_markdown(true)
            .execute_then_return_message(|args, response| match response {
                Ok(send_message::Response::Success(_)) => {}
                error => {
                    ic_cdk::println!("send_text_message: {args:?}, {error:?}");
                }
            }),
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

fn get_token_list() -> Vec<BotCommandOptionChoice<String>> {
    let mut ic = get_token_list_ic_ecosystem();

    ic.append(&mut get_token_list_other_ecosystem());

    ic
}

fn get_token_list_ic_ecosystem() -> Vec<BotCommandOptionChoice<String>> {
    let mut vec: Vec<BotCommandOptionChoice<String>> = vec![
        // BotCommandOptionChoice {
        //     name: "ICP".to_string(),
        //     value: "ICP_xrc".to_string(),
        // },
        BotCommandOptionChoice {
            name: "ICP".to_string(),
            value: "ICP_icpswap".to_string(),
        },
        BotCommandOptionChoice {
            name: "CHAT - OpenChat".to_string(),
            value: "CHAT".to_string(),
        },
        BotCommandOptionChoice {
            name: "ALICE".to_string(),
            value: "ALICE".to_string(),
        },
        BotCommandOptionChoice {
            name: "BOB".to_string(),
            value: "BOB".to_string(),
        },
        BotCommandOptionChoice {
            name: "AAA - aaaaa-aa ∞".to_string(),
            value: "AAA".to_string(),
        },
        BotCommandOptionChoice {
            name: "ALEX".to_string(),
            value: "ALEX".to_string(),
        },
        BotCommandOptionChoice {
            name: "CLOWN - Insane Clown Protocol".to_string(),
            value: "CLOWN".to_string(),
        },
        BotCommandOptionChoice {
            name: "RUGGY".to_string(),
            value: "RUGGY".to_string(),
        },
        BotCommandOptionChoice {
            name: "CLAY - Mimic Clay".to_string(),
            value: "CLAY".to_string(),
        },
        BotCommandOptionChoice {
            name: "GLDT - Gold Token".to_string(),
            value: "GLDT".to_string(),
        },
        BotCommandOptionChoice {
            name: "EXE - Windoge98".to_string(),
            value: "EXE".to_string(),
        },
        BotCommandOptionChoice {
            name: "CTZ - CatalyzeDAO".to_string(),
            value: "CLAY".to_string(),
        },
        BotCommandOptionChoice {
            name: "CECIL - Cecil The Lion DAO".to_string(),
            value: "CECIL".to_string(),
        },
        BotCommandOptionChoice {
            name: "DCD - DecideAI".to_string(),
            value: "DCD".to_string(),
        },
        BotCommandOptionChoice {
            name: "DOGMI".to_string(),
            value: "DOGMI".to_string(),
        },
        BotCommandOptionChoice {
            name: "DOLR - DOLR AI".to_string(),
            value: "DOLR".to_string(),
        },
        BotCommandOptionChoice {
            name: "DKP - Draggin Karma Points".to_string(),
            value: "DKP".to_string(),
        },
        BotCommandOptionChoice {
            name: "ELNA".to_string(),
            value: "ELNA".to_string(),
        },
        BotCommandOptionChoice {
            name: "EST - ESTATE".to_string(),
            value: "EST".to_string(),
        },
        BotCommandOptionChoice {
            name: "WELL - FomoWell".to_string(),
            value: "WELL".to_string(),
        },
        BotCommandOptionChoice {
            name: "FUEL".to_string(),
            value: "FUEL".to_string(),
        },
        BotCommandOptionChoice {
            name: "GHOST".to_string(),
            value: "GHOST".to_string(),
        },
        BotCommandOptionChoice {
            name: "GOLDAO".to_string(),
            value: "GOLDAO".to_string(),
        },
        BotCommandOptionChoice {
            name: "ICE - ICExplorer".to_string(),
            value: "ICE".to_string(),
        },
        BotCommandOptionChoice {
            name: "ICFC".to_string(),
            value: "ICFC".to_string(),
        },
        BotCommandOptionChoice {
            name: "ICL - ICLighthouse DAO".to_string(),
            value: "ICL".to_string(),
        },
        BotCommandOptionChoice {
            name: "PANDA - ICPanda".to_string(),
            value: "PANDA".to_string(),
        },
        BotCommandOptionChoice {
            name: "ICX - ICPEx".to_string(),
            value: "ICX".to_string(),
        },
        BotCommandOptionChoice {
            name: "ICS - ICPSwap Token".to_string(),
            value: "ICS".to_string(),
        },
        BotCommandOptionChoice {
            name: "ICVC".to_string(),
            value: "ICVC".to_string(),
        },
        BotCommandOptionChoice {
            name: "KINIC".to_string(),
            value: "KINIC".to_string(),
        },
        BotCommandOptionChoice {
            name: "KONG - KongSwap".to_string(),
            value: "KONG".to_string(),
        },
        BotCommandOptionChoice {
            name: "MOTOKO".to_string(),
            value: "MOTOKO".to_string(),
        },
        BotCommandOptionChoice {
            name: "NTN - Neutrinite".to_string(),
            value: "NTN".to_string(),
        },
        BotCommandOptionChoice {
            name: "NFIDW - NFID Wallet".to_string(),
            value: "NFIDW".to_string(),
        },
        BotCommandOptionChoice {
            name: "NUA - Nuance".to_string(),
            value: "NUA".to_string(),
        },
        BotCommandOptionChoice {
            name: "OGY - ORIGYN".to_string(),
            value: "OGY".to_string(),
        },
        BotCommandOptionChoice {
            name: "DAO - Personal DAO".to_string(),
            value: "DAO".to_string(),
        },
        BotCommandOptionChoice {
            name: "SNEED - Sneed DAO".to_string(),
            value: "DAO".to_string(),
        },
        BotCommandOptionChoice {
            name: "SONIC".to_string(),
            value: "SONIC".to_string(),
        },
        BotCommandOptionChoice {
            name: "SWAMP - SWAMPDAO".to_string(),
            value: "SWAMP".to_string(),
        },
        BotCommandOptionChoice {
            name: "TRAX".to_string(),
            value: "TRAX".to_string(),
        },
        BotCommandOptionChoice {
            name: "WTN - WaterNeuron".to_string(),
            value: "WTN".to_string(),
        },
        BotCommandOptionChoice {
            name: "YUKU - Yuku AI".to_string(),
            value: "YUKU".to_string(),
        },
        // BotCommandOptionChoice {
        //     name: "ckUSDC".to_string(),
        //     value: "ckUSDC".to_string(),
        // },
        // BotCommandOptionChoice {
        //     name: "ckETH".to_string(),
        //     value: "ckETH".to_string(),
        // },
        // BotCommandOptionChoice {
        //     name: "ckBTC".to_string(),
        //     value: "ckBTC".to_string(),
        // },
        // BotCommandOptionChoice {
        //     name: "ckLINK".to_string(),
        //     value: "ckLINK".to_string(),
        // },
        // BotCommandOptionChoice {
        //     name: "ckUSDT".to_string(),
        //     value: "ckUSDT".to_string(),
        // },
        // BotCommandOptionChoice {
        //     name: "ckOCT".to_string(),
        //     value: "ckOCT".to_string(),
        // },
        // BotCommandOptionChoice {
        //     name: "ckPEPE".to_string(),
        //     value: "ckPEPE".to_string(),
        // },
        // BotCommandOptionChoice {
        //     name: "ckXAUT".to_string(),
        //     value: "ckXAUT".to_string(),
        // },
        // BotCommandOptionChoice {
        //     name: "ckSHIB".to_string(),
        //     value: "ckSHIB".to_string(),
        // },
        // BotCommandOptionChoice {
        //     name: "ckWSTETH".to_string(),
        //     value: "ckWSTETH".to_string(),
        // },
        BotCommandOptionChoice {
            name: "XTC - Cycles".to_string(),
            value: "XTC".to_string(),
        },
        BotCommandOptionChoice {
            name: "SEER - Seers".to_string(),
            value: "SEER".to_string(),
        },
        BotCommandOptionChoice {
            name: "CLOUD - Crypto Cloud".to_string(),
            value: "CLOUD".to_string(),
        },
        BotCommandOptionChoice {
            name: "iDoge - Internet Doge".to_string(),
            value: "iDoge".to_string(),
        },
        BotCommandOptionChoice {
            name: "TAGGR".to_string(),
            value: "TAGGR".to_string(),
        },
    ];

    // vec.sort_by(|a, b| a.name.cmp(&b.name)); // sorting off

    return vec;
}

fn get_token_list_other_ecosystem() -> Vec<BotCommandOptionChoice<String>> {
    let mut vec: Vec<BotCommandOptionChoice<String>> = vec![
        BotCommandOptionChoice {
            name: "BTC - Bitcoin".to_string(),
            value: "BTC".to_string(),
        },
        BotCommandOptionChoice {
            name: "ETH - Ethereum".to_string(),
            value: "ETH".to_string(),
        },
        BotCommandOptionChoice {
            name: "USDT - Tether".to_string(),
            value: "USDT".to_string(),
        },
        BotCommandOptionChoice {
            name: "XRP".to_string(),
            value: "XRP".to_string(),
        },
        BotCommandOptionChoice {
            name: "BNB".to_string(),
            value: "BNB".to_string(),
        },
        BotCommandOptionChoice {
            name: "SOL - Solana".to_string(),
            value: "SOL".to_string(),
        },
        BotCommandOptionChoice {
            name: "USDC".to_string(),
            value: "USDC".to_string(),
        },
        BotCommandOptionChoice {
            name: "DOGE - Dogecoin".to_string(),
            value: "DOGE".to_string(),
        },
        BotCommandOptionChoice {
            name: "ADA - Cardano".to_string(),
            value: "ADA".to_string(),
        },
        BotCommandOptionChoice {
            name: "TRX".to_string(),
            value: "TRX".to_string(),
        },
        BotCommandOptionChoice {
            name: "SUI".to_string(),
            value: "SUI".to_string(),
        },
        BotCommandOptionChoice {
            name: "LINK - Chainlink".to_string(),
            value: "LINK".to_string(),
        },
        BotCommandOptionChoice {
            name: "AVAX - Avalanche".to_string(),
            value: "AVAX".to_string(),
        },
        BotCommandOptionChoice {
            name: "BCH - Bitcoin Cash".to_string(),
            value: "BCH".to_string(),
        },
        BotCommandOptionChoice {
            name: "XLM - Stellar".to_string(),
            value: "XLM".to_string(),
        },
        BotCommandOptionChoice {
            name: "LEO - UNUS SED LEO".to_string(),
            value: "LEO".to_string(),
        },
        BotCommandOptionChoice {
            name: "SHIB - Shiba Inu".to_string(),
            value: "SHIB".to_string(),
        },
        BotCommandOptionChoice {
            name: "HBAR - Hedera".to_string(),
            value: "HBAR".to_string(),
        },
        BotCommandOptionChoice {
            name: "TON - Toncoin".to_string(),
            value: "TON".to_string(),
        },
        BotCommandOptionChoice {
            name: "HYPE - Hyperliquid".to_string(),
            value: "HYPE".to_string(),
        },
        BotCommandOptionChoice {
            name: "LTC - Litecoin".to_string(),
            value: "LTC".to_string(),
        },
        BotCommandOptionChoice {
            name: "DOT - Polkadot".to_string(),
            value: "DOT".to_string(),
        },
        BotCommandOptionChoice {
            name: "DAI".to_string(),
            value: "DAI".to_string(),
        },
        BotCommandOptionChoice {
            name: "XMR - Monero".to_string(),
            value: "XMR".to_string(),
        },
        BotCommandOptionChoice {
            name: "BGB - Bitget Token".to_string(),
            value: "BGB".to_string(),
        },
        BotCommandOptionChoice {
            name: "USDe - Ethena USDe".to_string(),
            value: "USDe".to_string(),
        },
        BotCommandOptionChoice {
            name: "PI".to_string(),
            value: "PI".to_string(),
        },
        BotCommandOptionChoice {
            name: "PEPE".to_string(),
            value: "PEPE".to_string(),
        },
        BotCommandOptionChoice {
            name: "TAO - Bittensor".to_string(),
            value: "TAO".to_string(),
        },
        BotCommandOptionChoice {
            name: "UNI - Uniswap".to_string(),
            value: "UNI".to_string(),
        },
        BotCommandOptionChoice {
            name: "APT - Aptos".to_string(),
            value: "APT".to_string(),
        },
        BotCommandOptionChoice {
            name: "OKB".to_string(),
            value: "OKB".to_string(),
        },
        BotCommandOptionChoice {
            name: "NEAR - NEAR Protocol".to_string(),
            value: "NEAR".to_string(),
        },
        BotCommandOptionChoice {
            name: "ONDO".to_string(),
            value: "ONDO".to_string(),
        },
        BotCommandOptionChoice {
            name: "AAVE".to_string(),
            value: "AAVE".to_string(),
        },
        BotCommandOptionChoice {
            name: "GT - GateToken".to_string(),
            value: "GT".to_string(),
        },
        BotCommandOptionChoice {
            name: "ETC - Ethereum Classic".to_string(),
            value: "ETC".to_string(),
        },
        BotCommandOptionChoice {
            name: "ICP - Internet Computer".to_string(),
            value: "ICP".to_string(),
        },
        BotCommandOptionChoice {
            name: "CRO - Cronos".to_string(),
            value: "CRO".to_string(),
        },
        BotCommandOptionChoice {
            name: "TRUMP - OFFICIAL TRUMP".to_string(),
            value: "TRUMP".to_string(),
        },
        BotCommandOptionChoice {
            name: "KAS - Kaspa".to_string(),
            value: "KAS".to_string(),
        },
        BotCommandOptionChoice {
            name: "MNT - Mantle".to_string(),
            value: "MNT".to_string(),
        },
        BotCommandOptionChoice {
            name: "POL - POL (prev. MATIC)".to_string(),
            value: "POL".to_string(),
        },
        BotCommandOptionChoice {
            name: "RENDER".to_string(),
            value: "RENDER".to_string(),
        },
        BotCommandOptionChoice {
            name: "VET - VeChain".to_string(),
            value: "VET".to_string(),
        },
        BotCommandOptionChoice {
            name: "USD1 - World Liberty Financial USD".to_string(),
            value: "USD1".to_string(),
        },
        BotCommandOptionChoice {
            name: "FIL - Filecoin".to_string(),
            value: "FIL".to_string(),
        },
        BotCommandOptionChoice {
            name: "ALGO - Algorand".to_string(),
            value: "ALGO".to_string(),
        },
        BotCommandOptionChoice {
            name: "ENA - Ethena".to_string(),
            value: "ENA".to_string(),
        },
        BotCommandOptionChoice {
            name: "FET - Artificial Superintelligence Alliance".to_string(),
            value: "FET".to_string(),
        },
        BotCommandOptionChoice {
            name: "ATOM - Cosmos".to_string(),
            value: "ATOM".to_string(),
        },
        BotCommandOptionChoice {
            name: "TIA - Celestia".to_string(),
            value: "TIA".to_string(),
        },
        BotCommandOptionChoice {
            name: "ARB - Arbitrum".to_string(),
            value: "ARB".to_string(),
        },
        BotCommandOptionChoice {
            name: "S - Sonic (prev. FTM)".to_string(),
            value: "S".to_string(),
        },
        BotCommandOptionChoice {
            name: "FDUSD - First Digital USD".to_string(),
            value: "FDUSD".to_string(),
        },
        BotCommandOptionChoice {
            name: "STX - Stacks".to_string(),
            value: "STX".to_string(),
        },
        BotCommandOptionChoice {
            name: "BONK".to_string(),
            value: "BONK".to_string(),
        },
        BotCommandOptionChoice {
            name: "KCS - KuCoin Token".to_string(),
            value: "KCS".to_string(),
        },
        BotCommandOptionChoice {
            name: "WLD - Worldcoin".to_string(),
            value: "WLD".to_string(),
        },
        BotCommandOptionChoice {
            name: "MKR - Maker".to_string(),
            value: "MKR".to_string(),
        },
        BotCommandOptionChoice {
            name: "EOS".to_string(),
            value: "EOS".to_string(),
        },
        BotCommandOptionChoice {
            name: "FLR - Flare".to_string(),
            value: "FLR".to_string(),
        },
        BotCommandOptionChoice {
            name: "JUP - Jupiter".to_string(),
            value: "JUP".to_string(),
        },
        BotCommandOptionChoice {
            name: "DEXE".to_string(),
            value: "DEXE".to_string(),
        },
        BotCommandOptionChoice {
            name: "XDC - XDC Network".to_string(),
            value: "XDC".to_string(),
        },
        BotCommandOptionChoice {
            name: "QNT - Quant".to_string(),
            value: "QNT".to_string(),
        },
        BotCommandOptionChoice {
            name: "IP - Story".to_string(),
            value: "IP".to_string(),
        },
        BotCommandOptionChoice {
            name: "FARTCOIN".to_string(),
            value: "FARTCOIN".to_string(),
        },
        BotCommandOptionChoice {
            name: "SEI".to_string(),
            value: "SEI".to_string(),
        },
        BotCommandOptionChoice {
            name: "IMX - Immutable".to_string(),
            value: "IMX".to_string(),
        },
        BotCommandOptionChoice {
            name: "OP - Optimism".to_string(),
            value: "OP".to_string(),
        },
        BotCommandOptionChoice {
            name: "INJ - Injective".to_string(),
            value: "INJ".to_string(),
        },
        BotCommandOptionChoice {
            name: "VIRTUAL - Virtuals Protocol".to_string(),
            value: "VIRTUAL".to_string(),
        },
        BotCommandOptionChoice {
            name: "FORM - Four".to_string(),
            value: "FORM".to_string(),
        },
        BotCommandOptionChoice {
            name: "CRV - Curve DAO Token".to_string(),
            value: "CRV".to_string(),
        },
        BotCommandOptionChoice {
            name: "GRT - The Graph".to_string(),
            value: "GRT".to_string(),
        },
        BotCommandOptionChoice {
            name: "PYUSD - PayPal USD".to_string(),
            value: "PYUSD".to_string(),
        },
        BotCommandOptionChoice {
            name: "XAUt - Tether Gold".to_string(),
            value: "XAUt".to_string(),
        },
        BotCommandOptionChoice {
            name: "NEXO".to_string(),
            value: "NEXO".to_string(),
        },
        BotCommandOptionChoice {
            name: "JASMY - JasmyCoin".to_string(),
            value: "JASMY".to_string(),
        },
        BotCommandOptionChoice {
            name: "PAXG - PAX Gold".to_string(),
            value: "PAXG".to_string(),
        },
        BotCommandOptionChoice {
            name: "IOTA".to_string(),
            value: "IOTA".to_string(),
        },
        BotCommandOptionChoice {
            name: "WAL - Walrus".to_string(),
            value: "WAL".to_string(),
        },
        BotCommandOptionChoice {
            name: "FLOKI".to_string(),
            value: "FLOKI".to_string(),
        },
        BotCommandOptionChoice {
            name: "THETA - Theta Network".to_string(),
            value: "THETA".to_string(),
        },
        BotCommandOptionChoice {
            name: "BSV - Bitcoin SV".to_string(),
            value: "BSV".to_string(),
        },
        BotCommandOptionChoice {
            name: "RAY - Raydium".to_string(),
            value: "RAY".to_string(),
        },
        BotCommandOptionChoice {
            name: "LDO - Lido DAO".to_string(),
            value: "LDO".to_string(),
        },
        BotCommandOptionChoice {
            name: "PENGU - Pudgy Penguins".to_string(),
            value: "PENGU".to_string(),
        },
        BotCommandOptionChoice {
            name: "SAND - The Sandbox".to_string(),
            value: "SAND".to_string(),
        },
        BotCommandOptionChoice {
            name: "CORE".to_string(),
            value: "CORE".to_string(),
        },
        BotCommandOptionChoice {
            name: "GALA".to_string(),
            value: "GALA".to_string(),
        },
        BotCommandOptionChoice {
            name: "BTT - BitTorrent [New]".to_string(),
            value: "BTT".to_string(),
        },
        BotCommandOptionChoice {
            name: "ENS - Ethereum Name Service".to_string(),
            value: "ENS".to_string(),
        },
        BotCommandOptionChoice {
            name: "KAIA".to_string(),
            value: "KAIA".to_string(),
        },
        BotCommandOptionChoice {
            name: "HNT - Helium".to_string(),
            value: "HNT".to_string(),
        },
        BotCommandOptionChoice {
            name: "CAKE - PancakeSwap".to_string(),
            value: "CAKE".to_string(),
        },
        BotCommandOptionChoice {
            name: "ZEC - Zcash".to_string(),
            value: "ZEC".to_string(),
        },
        BotCommandOptionChoice {
            name: "ZKJ - Polyhedra Network".to_string(),
            value: "ZKJ".to_string(),
        },
        BotCommandOptionChoice {
            name: "FLOW".to_string(),
            value: "FLOW".to_string(),
        },
        BotCommandOptionChoice {
            name: "MANA - Decentraland".to_string(),
            value: "MANA".to_string(),
        },
        BotCommandOptionChoice {
            name: "WIF - dogwifhat".to_string(),
            value: "WIF".to_string(),
        },
        BotCommandOptionChoice {
            name: "BRETT - Brett (Based)".to_string(),
            value: "BRETT".to_string(),
        },
        BotCommandOptionChoice {
            name: "DEEP - DeepBook Protocol".to_string(),
            value: "DEEP".to_string(),
        },
        BotCommandOptionChoice {
            name: "XCN - Onyxcoin".to_string(),
            value: "XCN".to_string(),
        },
        BotCommandOptionChoice {
            name: "XTZ - Tezos".to_string(),
            value: "XTZ".to_string(),
        },
        BotCommandOptionChoice {
            name: "PENDLE".to_string(),
            value: "PENDLE".to_string(),
        },
        BotCommandOptionChoice {
            name: "JTO - Jito".to_string(),
            value: "JTO".to_string(),
        },
        BotCommandOptionChoice {
            name: "AERO - Aerodrome Finance".to_string(),
            value: "AERO".to_string(),
        },
        BotCommandOptionChoice {
            name: "SPX - SPX6900".to_string(),
            value: "SPX".to_string(),
        },
        BotCommandOptionChoice {
            name: "PYTH - Pyth Network".to_string(),
            value: "PYTH".to_string(),
        },
        BotCommandOptionChoice {
            name: "RSR - Reserve Rights".to_string(),
            value: "RSR".to_string(),
        },
        BotCommandOptionChoice {
            name: "TUSD - TrueUSDTUSD".to_string(),
            value: "TUSD".to_string(),
        },
        BotCommandOptionChoice {
            name: "KAVA".to_string(),
            value: "KAVA".to_string(),
        },
        BotCommandOptionChoice {
            name: "AIOZ - AIOZ Network".to_string(),
            value: "AIOZ".to_string(),
        },
        BotCommandOptionChoice {
            name: "AR - Arweave".to_string(),
            value: "AR".to_string(),
        },
        BotCommandOptionChoice {
            name: "RUNE - THORChain".to_string(),
            value: "RUNE".to_string(),
        },
        BotCommandOptionChoice {
            name: "DYDX".to_string(),
            value: "DYDX".to_string(),
        },
        BotCommandOptionChoice {
            name: "EGLD - MultiversXEGLD".to_string(),
            value: "EGLD".to_string(),
        },
        BotCommandOptionChoice {
            name: "POPCAT - Popcat (SOL)".to_string(),
            value: "POPCAT".to_string(),
        },
        BotCommandOptionChoice {
            name: "XEC - eCash".to_string(),
            value: "XEC".to_string(),
        },
        BotCommandOptionChoice {
            name: "ABAB".to_string(),
            value: "ABAB".to_string(),
        },
        BotCommandOptionChoice {
            name: "STRK - Starknet".to_string(),
            value: "STRK".to_string(),
        },
        BotCommandOptionChoice {
            name: "NFT - APENFT".to_string(),
            value: "NFT".to_string(),
        },
        BotCommandOptionChoice {
            name: "NEO".to_string(),
            value: "NEO".to_string(),
        },
        BotCommandOptionChoice {
            name: "SUPER - SuperVerse".to_string(),
            value: "SUPER".to_string(),
        },
        BotCommandOptionChoice {
            name: "AXS - Axie Infinity".to_string(),
            value: "AXS".to_string(),
        },
        BotCommandOptionChoice {
            name: "MOVE - Movement".to_string(),
            value: "MOVE".to_string(),
        },
        BotCommandOptionChoice {
            name: "AKT - Akash Network".to_string(),
            value: "AKT".to_string(),
        },
        BotCommandOptionChoice {
            name: "CHZ - Chiliz".to_string(),
            value: "CHZ".to_string(),
        },
        BotCommandOptionChoice {
            name: "BERA - Berachain".to_string(),
            value: "BERA".to_string(),
        },
        BotCommandOptionChoice {
            name: "APE - ApeCoin".to_string(),
            value: "APE".to_string(),
        },
        BotCommandOptionChoice {
            name: "BEAM".to_string(),
            value: "BEAM".to_string(),
        },
        BotCommandOptionChoice {
            name: "CFX - Conflux".to_string(),
            value: "CFX".to_string(),
        },
        BotCommandOptionChoice {
            name: "W - Wormhole".to_string(),
            value: "W".to_string(),
        },
        BotCommandOptionChoice {
            name: "TURBO".to_string(),
            value: "TURBO".to_string(),
        },
        BotCommandOptionChoice {
            name: "GRASS".to_string(),
            value: "GRASS".to_string(),
        },
        BotCommandOptionChoice {
            name: "MORPHO".to_string(),
            value: "MORPHO".to_string(),
        },
        BotCommandOptionChoice {
            name: "AXL - Axelar".to_string(),
            value: "AXL".to_string(),
        },
        BotCommandOptionChoice {
            name: "COMP - Compound".to_string(),
            value: "COMP".to_string(),
        },
        BotCommandOptionChoice {
            name: "KAITO".to_string(),
            value: "KAITO".to_string(),
        },
        BotCommandOptionChoice {
            name: "OM - MANTRA".to_string(),
            value: "OM".to_string(),
        },
        BotCommandOptionChoice {
            name: "SUN - Sun [New]".to_string(),
            value: "SUN".to_string(),
        },
        BotCommandOptionChoice {
            name: "FTT - FTX Token".to_string(),
            value: "FTT".to_string(),
        },
        BotCommandOptionChoice {
            name: "JST - JUST".to_string(),
            value: "JST".to_string(),
        },
        BotCommandOptionChoice {
            name: "MOG - Mog Coin".to_string(),
            value: "MOG".to_string(),
        },
        BotCommandOptionChoice {
            name: "AMP".to_string(),
            value: "AMP".to_string(),
        },
        BotCommandOptionChoice {
            name: "USDD".to_string(),
            value: "USDD".to_string(),
        },
        BotCommandOptionChoice {
            name: "SAFE".to_string(),
            value: "SAFE".to_string(),
        },
        BotCommandOptionChoice {
            name: "LUNC - Terra Classic".to_string(),
            value: "LUNC".to_string(),
        },
        BotCommandOptionChoice {
            name: "RON - Ronin".to_string(),
            value: "RON".to_string(),
        },
        BotCommandOptionChoice {
            name: "TWT - Trust Wallet Token".to_string(),
            value: "TWT".to_string(),
        },
        BotCommandOptionChoice {
            name: "CVX - Convex Finance".to_string(),
            value: "CVX".to_string(),
        },
        BotCommandOptionChoice {
            name: "CTC - Creditcoin".to_string(),
            value: "CTC".to_string(),
        },
        BotCommandOptionChoice {
            name: "AI16Z".to_string(),
            value: "AI16Z".to_string(),
        },
        BotCommandOptionChoice {
            name: "GNO - Gnosis".to_string(),
            value: "GNO".to_string(),
        },
        BotCommandOptionChoice {
            name: "DOG - Dog (Bitcoin)".to_string(),
            value: "DOG".to_string(),
        },
        BotCommandOptionChoice {
            name: "LAYER - Solayer".to_string(),
            value: "LAYER".to_string(),
        },
        BotCommandOptionChoice {
            name: "ZRO - LayerZero".to_string(),
            value: "ZRO".to_string(),
        },
        BotCommandOptionChoice {
            name: "MINA".to_string(),
            value: "MINA".to_string(),
        },
        BotCommandOptionChoice {
            name: "1INCH - 1inch Network".to_string(),
            value: "1INCH".to_string(),
        },
        BotCommandOptionChoice {
            name: "CHEEMS - Cheems (cheems.pet)".to_string(),
            value: "CHEEMS".to_string(),
        },
        BotCommandOptionChoice {
            name: "ATH - Aethir".to_string(),
            value: "ATH".to_string(),
        },
        BotCommandOptionChoice {
            name: "DASH".to_string(),
            value: "DASH".to_string(),
        },
        BotCommandOptionChoice {
            name: "SFP - SafePal".to_string(),
            value: "SFP".to_string(),
        },
        BotCommandOptionChoice {
            name: "GLM - Golem".to_string(),
            value: "GLM".to_string(),
        },
        BotCommandOptionChoice {
            name: "MEW - cat in a dogs world".to_string(),
            value: "MEW".to_string(),
        },
        BotCommandOptionChoice {
            name: "KSM - Kusama".to_string(),
            value: "KSM".to_string(),
        },
        BotCommandOptionChoice {
            name: "TFUEL - Theta Fuel".to_string(),
            value: "TFUEL".to_string(),
        },
        BotCommandOptionChoice {
            name: "MX - MX Token".to_string(),
            value: "MX".to_string(),
        },
        BotCommandOptionChoice {
            name: "ZIL - Zilliqa".to_string(),
            value: "ZIL".to_string(),
        },
        BotCommandOptionChoice {
            name: "SYRUP - Maple Finance".to_string(),
            value: "SYRUP".to_string(),
        },
        BotCommandOptionChoice {
            name: "MOCA - Moca Network".to_string(),
            value: "MOCA".to_string(),
        },
        BotCommandOptionChoice {
            name: "BLUR".to_string(),
            value: "BLUR".to_string(),
        },
        BotCommandOptionChoice {
            name: "QTUM".to_string(),
            value: "QTUM".to_string(),
        },
        BotCommandOptionChoice {
            name: "ACH - Alchemy Pay".to_string(),
            value: "ACH".to_string(),
        },
        BotCommandOptionChoice {
            name: "NOT - Notcoin".to_string(),
            value: "NOT".to_string(),
        },
        BotCommandOptionChoice {
            name: "EIGEN - EigenLayer".to_string(),
            value: "EIGEN".to_string(),
        },
        BotCommandOptionChoice {
            name: "SNX - Synthetix".to_string(),
            value: "SNX".to_string(),
        },
        BotCommandOptionChoice {
            name: "VTHO - VeThor Token".to_string(),
            value: "VTHO".to_string(),
        },
        BotCommandOptionChoice {
            name: "DCR - Decred".to_string(),
            value: "DCR".to_string(),
        },
        BotCommandOptionChoice {
            name: "ZRX - 0x Protocol".to_string(),
            value: "ZRX".to_string(),
        },
        BotCommandOptionChoice {
            name: "CKB - Nervos Network".to_string(),
            value: "CKB".to_string(),
        },
        BotCommandOptionChoice {
            name: "ZETA - ZetaChain".to_string(),
            value: "ZETA".to_string(),
        },
        BotCommandOptionChoice {
            name: "BAT - Basic Attention Token".to_string(),
            value: "BAT".to_string(),
        },
        BotCommandOptionChoice {
            name: "BabyDoge - Baby Doge Coin".to_string(),
            value: "BabyDoge".to_string(),
        },
        BotCommandOptionChoice {
            name: "ASTR".to_string(),
            value: "ASTR".to_string(),
        },
        BotCommandOptionChoice {
            name: "GAS".to_string(),
            value: "GAS".to_string(),
        },
        BotCommandOptionChoice {
            name: "TRAC - OriginTrail".to_string(),
            value: "TRAC".to_string(),
        },
        BotCommandOptionChoice {
            name: "ID - SPACE ID".to_string(),
            value: "ID".to_string(),
        },
        BotCommandOptionChoice {
            name: "ROSE - Oasis".to_string(),
            value: "ROSE".to_string(),
        },
        BotCommandOptionChoice {
            name: "ZK - ZKsync".to_string(),
            value: "ZK".to_string(),
        },
        BotCommandOptionChoice {
            name: "DRIFT".to_string(),
            value: "DRIFT".to_string(),
        },
        BotCommandOptionChoice {
            name: "KDA - Kadena".to_string(),
            value: "KDA".to_string(),
        },
        BotCommandOptionChoice {
            name: "CELO".to_string(),
            value: "CELO".to_string(),
        },
        BotCommandOptionChoice {
            name: "LPT - Livepeer".to_string(),
            value: "LPT".to_string(),
        },
        BotCommandOptionChoice {
            name: "BABY - Babylon".to_string(),
            value: "BABY".to_string(),
        },
        BotCommandOptionChoice {
            name: "FRAX - Frax (prev. FXS)".to_string(),
            value: "FRAX".to_string(),
        },
        BotCommandOptionChoice {
            name: "ONE - Harmony".to_string(),
            value: "ONE".to_string(),
        },
        BotCommandOptionChoice {
            name: "ANKR".to_string(),
            value: "ANKR".to_string(),
        },
        BotCommandOptionChoice {
            name: "FTN - Fasttoken".to_string(),
            value: "FTN".to_string(),
        },
        BotCommandOptionChoice {
            name: "JLP - Jupiter Perps LP".to_string(),
            value: "JLP".to_string(),
        },
        BotCommandOptionChoice {
            name: "ULTIMA".to_string(),
            value: "ULTIMA".to_string(),
        },
        BotCommandOptionChoice {
            name: "FLZ - Fellaz".to_string(),
            value: "FLZ".to_string(),
        },
        BotCommandOptionChoice {
            name: "USD0 - Usual USD".to_string(),
            value: "USD0".to_string(),
        },
        BotCommandOptionChoice {
            name: "USDY - Ondo US Dollar Yield".to_string(),
            value: "USDY".to_string(),
        },
        BotCommandOptionChoice {
            name: "ZBU - Zeebu".to_string(),
            value: "ZBU".to_string(),
        },
        BotCommandOptionChoice {
            name: "WOULD".to_string(),
            value: "WOULD".to_string(),
        },
        BotCommandOptionChoice {
            name: "TEL - Telcoin".to_string(),
            value: "TEL".to_string(),
        },
        BotCommandOptionChoice {
            name: "BDX - Beldex".to_string(),
            value: "BDX".to_string(),
        },
        BotCommandOptionChoice {
            name: "OHM - Olympus v2".to_string(),
            value: "OHM".to_string(),
        },
        BotCommandOptionChoice {
            name: "UPC - UPCX".to_string(),
            value: "UPC".to_string(),
        },
        BotCommandOptionChoice {
            name: "PLUME".to_string(),
            value: "PLUME".to_string(),
        },
        BotCommandOptionChoice {
            name: "WHITE - WhiteRock".to_string(),
            value: "WHITE".to_string(),
        },
        BotCommandOptionChoice {
            name: "CHEEL - Cheelee".to_string(),
            value: "CHEEL".to_string(),
        },
        BotCommandOptionChoice {
            name: "rLUSD - Ripple USD".to_string(),
            value: "rLUSD".to_string(),
        },
        BotCommandOptionChoice {
            name: "FRAX - Legacy Frax Dollar".to_string(),
            value: "FRAX".to_string(),
        },
        BotCommandOptionChoice {
            name: "USDf - Falcon USD".to_string(),
            value: "USDf".to_string(),
        },
        BotCommandOptionChoice {
            name: "SNEK".to_string(),
            value: "SNEK".to_string(),
        },
        BotCommandOptionChoice {
            name: "GHO".to_string(),
            value: "GHO".to_string(),
        },
        BotCommandOptionChoice {
            name: "EURC".to_string(),
            value: "EURC".to_string(),
        },
        BotCommandOptionChoice {
            name: "USDG - Global Dollar".to_string(),
            value: "USDG".to_string(),
        },
        BotCommandOptionChoice {
            name: "BORG - SwissBorg".to_string(),
            value: "BORG".to_string(),
        },
        BotCommandOptionChoice {
            name: "LGCT - Legacy Token".to_string(),
            value: "LGCT".to_string(),
        },
        BotCommandOptionChoice {
            name: "CHEX - Chintai".to_string(),
            value: "CHEX".to_string(),
        },
        BotCommandOptionChoice {
            name: "ALE - Ailey".to_string(),
            value: "ALE".to_string(),
        },
        BotCommandOptionChoice {
            name: "MELANIA - Official Melania Meme".to_string(),
            value: "MELANIA".to_string(),
        },
        BotCommandOptionChoice {
            name: "SC - Siacoin".to_string(),
            value: "SC".to_string(),
        },
        BotCommandOptionChoice {
            name: "UXLINK".to_string(),
            value: "UXLINK".to_string(),
        },
        BotCommandOptionChoice {
            name: "YFI - yearn.finance".to_string(),
            value: "YFI".to_string(),
        },
        BotCommandOptionChoice {
            name: "QUBIC".to_string(),
            value: "QUBIC".to_string(),
        },
        BotCommandOptionChoice {
            name: "SAROS".to_string(),
            value: "SAROS".to_string(),
        },
        BotCommandOptionChoice {
            name: "T - Threshold".to_string(),
            value: "T".to_string(),
        },
        BotCommandOptionChoice {
            name: "DEUSD - Elixir deUSD".to_string(),
            value: "DEUSD".to_string(),
        },
        BotCommandOptionChoice {
            name: "ETHW - EthereumPoW".to_string(),
            value: "ETHW".to_string(),
        },
        BotCommandOptionChoice {
            name: "CSPR - Casper".to_string(),
            value: "CSPR".to_string(),
        },
        BotCommandOptionChoice {
            name: "GIGA - Gigachad".to_string(),
            value: "GIGA".to_string(),
        },
        BotCommandOptionChoice {
            name: "ELF - aelf".to_string(),
            value: "ELF".to_string(),
        },
        BotCommandOptionChoice {
            name: "HOT - Holo".to_string(),
            value: "HOT".to_string(),
        },
        BotCommandOptionChoice {
            name: "SOS - Solana Swap".to_string(),
            value: "SOS".to_string(),
        },
        BotCommandOptionChoice {
            name: "AIXBT".to_string(),
            value: "AIXBT".to_string(),
        },
        BotCommandOptionChoice {
            name: "RVN - Ravencoin".to_string(),
            value: "RVN".to_string(),
        },
        BotCommandOptionChoice {
            name: "IOTX - IoTeX".to_string(),
            value: "IOTX".to_string(),
        },
        BotCommandOptionChoice {
            name: "XYO".to_string(),
            value: "XYO".to_string(),
        },
        BotCommandOptionChoice {
            name: "SUSHI - SushiSwap".to_string(),
            value: "SUSHI".to_string(),
        },
        BotCommandOptionChoice {
            name: "VANA".to_string(),
            value: "VANA".to_string(),
        },
        BotCommandOptionChoice {
            name: "GOMINING".to_string(),
            value: "GOMINING".to_string(),
        },
        BotCommandOptionChoice {
            name: "PNUT - Peanut the Squirrel".to_string(),
            value: "PNUT".to_string(),
        },
        BotCommandOptionChoice {
            name: "SQD - Subsquid".to_string(),
            value: "SQD".to_string(),
        },
        BotCommandOptionChoice {
            name: "HMSTR - Hamster Kombat".to_string(),
            value: "HMSTR".to_string(),
        },
        BotCommandOptionChoice {
            name: "DGB - DigiByte".to_string(),
            value: "DGB".to_string(),
        },
        BotCommandOptionChoice {
            name: "USDO - OpenEden OpenDollar".to_string(),
            value: "USDO".to_string(),
        },
        BotCommandOptionChoice {
            name: "OSMO - Osmosis".to_string(),
            value: "OSMO".to_string(),
        },
        BotCommandOptionChoice {
            name: "XCH - Chia".to_string(),
            value: "XCH".to_string(),
        },
        BotCommandOptionChoice {
            name: "XEM - NEM".to_string(),
            value: "XEM".to_string(),
        },
        BotCommandOptionChoice {
            name: "KOGE - 48 Club Token".to_string(),
            value: "KOGE".to_string(),
        },
        BotCommandOptionChoice {
            name: "TOSHI".to_string(),
            value: "TOSHI".to_string(),
        },
        BotCommandOptionChoice {
            name: "COTI".to_string(),
            value: "COTI".to_string(),
        },
        BotCommandOptionChoice {
            name: "WEMIX".to_string(),
            value: "WEMIX".to_string(),
        },
        BotCommandOptionChoice {
            name: "GMT".to_string(),
            value: "GMT".to_string(),
        },
        BotCommandOptionChoice {
            name: "MPLX - Metaplex".to_string(),
            value: "MPLX".to_string(),
        },
        BotCommandOptionChoice {
            name: "ORDI".to_string(),
            value: "ORDI".to_string(),
        },
        BotCommandOptionChoice {
            name: "CETUS - Cetus Protocol".to_string(),
            value: "CETUS".to_string(),
        },
        BotCommandOptionChoice {
            name: "DLC - Diverge Loop".to_string(),
            value: "DLC".to_string(),
        },
        BotCommandOptionChoice {
            name: "ZANO".to_string(),
            value: "ZANO".to_string(),
        },
        BotCommandOptionChoice {
            name: "STPT".to_string(),
            value: "STPT".to_string(),
        },
        BotCommandOptionChoice {
            name: "ORCA".to_string(),
            value: "ORCA".to_string(),
        },
        BotCommandOptionChoice {
            name: "ALCH - Alchemist AI".to_string(),
            value: "ALCH".to_string(),
        },
        BotCommandOptionChoice {
            name: "ME - Magic Eden".to_string(),
            value: "ME".to_string(),
        },
        BotCommandOptionChoice {
            name: "ETHFI - ether.fi".to_string(),
            value: "ETHFI".to_string(),
        },
        BotCommandOptionChoice {
            name: "G - Gravity (by Galxe)".to_string(),
            value: "G".to_string(),
        },
        BotCommandOptionChoice {
            name: "POLYX - Polymesh".to_string(),
            value: "POLYX".to_string(),
        },
        BotCommandOptionChoice {
            name: "EUL - Euler".to_string(),
            value: "EUL".to_string(),
        },
        BotCommandOptionChoice {
            name: "AIC - AI Companions".to_string(),
            value: "AIC".to_string(),
        },
        BotCommandOptionChoice {
            name: "LRC - Loopring".to_string(),
            value: "LRC".to_string(),
        },
        BotCommandOptionChoice {
            name: "EURS - STASIS EURO".to_string(),
            value: "EURS".to_string(),
        },
        BotCommandOptionChoice {
            name: "RLB - Rollbit Coin".to_string(),
            value: "RLB".to_string(),
        },
        BotCommandOptionChoice {
            name: "ENJ - Enjin Coin".to_string(),
            value: "ENJ".to_string(),
        },
        BotCommandOptionChoice {
            name: "STIK - Staika".to_string(),
            value: "STIK".to_string(),
        },
        BotCommandOptionChoice {
            name: "GMX".to_string(),
            value: "GMX".to_string(),
        },
        BotCommandOptionChoice {
            name: "SWFTC - SwftCoin".to_string(),
            value: "SWFTC".to_string(),
        },
        BotCommandOptionChoice {
            name: "ZEN - Horizen".to_string(),
            value: "ZEN".to_string(),
        },
        BotCommandOptionChoice {
            name: "FAI - Freysa".to_string(),
            value: "FAI".to_string(),
        },
        BotCommandOptionChoice {
            name: "BIGTIME".to_string(),
            value: "BIGTIME".to_string(),
        },
        BotCommandOptionChoice {
            name: "ONT - Ontology".to_string(),
            value: "ONT".to_string(),
        },
        BotCommandOptionChoice {
            name: "LCX".to_string(),
            value: "LCX".to_string(),
        },
        BotCommandOptionChoice {
            name: "WAVES".to_string(),
            value: "WAVES".to_string(),
        },
        BotCommandOptionChoice {
            name: "WOO".to_string(),
            value: "WOO".to_string(),
        },
        BotCommandOptionChoice {
            name: "DAG - Constellation".to_string(),
            value: "DAG".to_string(),
        },
        BotCommandOptionChoice {
            name: "ZBCN - Zebec Network".to_string(),
            value: "ZBCN".to_string(),
        },
        BotCommandOptionChoice {
            name: "DSYNC - Destra Network".to_string(),
            value: "DSYNC".to_string(),
        },
        BotCommandOptionChoice {
            name: "SKL - SKALE".to_string(),
            value: "SKL".to_string(),
        },
        BotCommandOptionChoice {
            name: "SXP - Solar".to_string(),
            value: "SXP".to_string(),
        },
        BotCommandOptionChoice {
            name: "GOHOME".to_string(),
            value: "GOHOME".to_string(),
        },
        BotCommandOptionChoice {
            name: "FUSDF - Aster USD".to_string(),
            value: "FUSDF".to_string(),
        },
        BotCommandOptionChoice {
            name: "BAND - Band Protocol".to_string(),
            value: "BAND".to_string(),
        },
        BotCommandOptionChoice {
            name: "COW - CoW Protocol".to_string(),
            value: "COW".to_string(),
        },
        BotCommandOptionChoice {
            name: "LUNA - Terra".to_string(),
            value: "LUNA".to_string(),
        },
        BotCommandOptionChoice {
            name: "HIVE".to_string(),
            value: "HIVE".to_string(),
        },
        BotCommandOptionChoice {
            name: "WMTX - World Mobile Token".to_string(),
            value: "WMTX".to_string(),
        },
    ];

    // vec.sort_by(|a, b| a.name.cmp(&b.name)); // sorting off

    return vec;
}

fn get_tokens_map() -> HashMap<String, Config> {
    // IC ecosystem
    let mut tokenlist: Vec<(String, Config)> = vec![
        (
            "ICP_xrc".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ICP".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ICP_icpswap".to_string(),
            Config::from_canister_id_str("ryjl3-tyaaa-aaaaa-aaaba-cai"),
        ),
        (
            "CHAT".to_string(),
            Config::from_canister_id_str("2ouva-viaaa-aaaaq-aaamq-cai"),
        ),
        (
            "ALICE".to_string(),
            Config::from_canister_id_str("oj6if-riaaa-aaaaq-aaeha-cai"),
        ),
        (
            "CLAY".to_string(),
            Config::from_canister_id_str("vtrom-gqaaa-aaaaq-aabia-cai"),
        ),
        (
            "CTZ".to_string(),
            Config::from_canister_id_str("uf2wh-taaaa-aaaaq-aabna-cai"),
        ),
        (
            "CECIL".to_string(),
            Config::from_canister_id_str("jg2ra-syaaa-aaaaq-aaewa-cai"),
        ),
        (
            "DCD".to_string(),
            Config::from_canister_id_str("xsi2v-cyaaa-aaaaq-aabfq-cai"),
        ),
        (
            "DOGMI".to_string(),
            Config::from_canister_id_str("np5km-uyaaa-aaaaq-aadrq-cai"),
        ),
        (
            "DOLR".to_string(),
            Config::from_canister_id_str("6rdgd-kyaaa-aaaaq-aaavq-cai"),
        ),
        (
            "DKP".to_string(),
            Config::from_canister_id_str("zfcdd-tqaaa-aaaaq-aaaga-cai"),
        ),
        (
            "ELNA".to_string(),
            Config::from_canister_id_str("gemj7-oyaaa-aaaaq-aacnq-cai"),
        ),
        (
            "EST".to_string(),
            Config::from_canister_id_str("bliq2-niaaa-aaaaq-aac4q-cai"),
        ),
        (
            "WELL".to_string(),
            Config::from_canister_id_str("o4zzi-qaaaa-aaaaq-aaeeq-cai"),
        ),
        (
            "FUEL".to_string(),
            Config::from_canister_id_str("nfjys-2iaaa-aaaaq-aaena-cai"),
        ),
        (
            "GHOST".to_string(),
            Config::from_canister_id_str("4c4fd-caaaa-aaaaq-aaa3a-cai"),
        ),
        (
            "GOLDAO".to_string(),
            Config::from_canister_id_str("tyyy3-4aaaa-aaaaq-aab7a-cai"),
        ),
        (
            "ICE".to_string(),
            Config::from_canister_id_str("ifwyg-gaaaa-aaaaq-aaeqq-cai"),
        ),
        (
            "ICFC".to_string(),
            Config::from_canister_id_str("ddsp7-7iaaa-aaaaq-aacqq-cai"),
        ),
        (
            "ICL".to_string(),
            Config::from_canister_id_str("hhaaz-2aaaa-aaaaq-aacla-cai"),
        ),
        (
            "PANDA".to_string(),
            Config::from_canister_id_str("druyg-tyaaa-aaaaq-aactq-cai"),
        ),
        (
            "ICX".to_string(),
            Config::from_canister_id_str("lvfsa-2aaaa-aaaaq-aaeyq-cai"),
        ),
        (
            "ICS".to_string(),
            Config::from_canister_id_str("ca6gz-lqaaa-aaaaq-aacwa-cai"),
        ),
        (
            "ICVC".to_string(),
            Config::from_canister_id_str("m6xut-mqaaa-aaaaq-aadua-cai"),
        ),
        (
            "KINIC".to_string(),
            Config::from_canister_id_str("73mez-iiaaa-aaaaq-aaasq-cai"),
        ),
        (
            "KONG".to_string(),
            Config::from_canister_id_str("o7oak-iyaaa-aaaaq-aadzq-cai"),
        ),
        (
            "MOTOKO".to_string(),
            Config::from_canister_id_str("k45jy-aiaaa-aaaaq-aadcq-cai"),
        ),
        (
            "NTN".to_string(),
            Config::from_canister_id_str("f54if-eqaaa-aaaaq-aacea-cai"),
        ),
        (
            "NFIDW".to_string(),
            Config::from_canister_id_str("mih44-vaaaa-aaaaq-aaekq-cai"),
        ),
        (
            "NUA".to_string(),
            Config::from_canister_id_str("rxdbk-dyaaa-aaaaq-aabtq-cai"),
        ),
        (
            "OGY".to_string(),
            Config::from_canister_id_str("lkwrt-vyaaa-aaaaq-aadhq-cai"),
        ),
        (
            "DAO".to_string(),
            Config::from_canister_id_str("ixqp7-kqaaa-aaaaq-aaetq-cai"),
        ),
        (
            "SNEED".to_string(),
            Config::from_canister_id_str("hvgxa-wqaaa-aaaaq-aacia-cai"),
        ),
        (
            "SONIC".to_string(),
            Config::from_canister_id_str("73mez-iiaaa-aaaaq-aaasq-cai"),
        ),
        (
            "SONIC".to_string(),
            Config::from_canister_id_str("qbizb-wiaaa-aaaaq-aabwq-cai"),
        ),
        (
            "SWAMP".to_string(),
            Config::from_canister_id_str("lrtnw-paaaa-aaaaq-aadfa-cai"),
        ),
        (
            "TRAX".to_string(),
            Config::from_canister_id_str("emww2-4yaaa-aaaaq-aacbq-cai"),
        ),
        (
            "WTN".to_string(),
            Config::from_canister_id_str("jcmow-hyaaa-aaaaq-aadlq-cai"),
        ),
        (
            "YUKU".to_string(),
            Config::from_canister_id_str("atbfz-diaaa-aaaaq-aacyq-cai"),
        ),
        (
            "ckUSDC".to_string(),
            Config::from_canister_id_str("xevnm-gaaaa-aaaar-qafnq-cai"),
        ),
        (
            "ckETH".to_string(),
            Config::from_canister_id_str("ss2fx-dyaaa-aaaar-qacoq-cai"),
        ),
        (
            "ckBTC".to_string(),
            Config::from_canister_id_str("mxzaz-hqaaa-aaaar-qaada-cai"),
        ),
        (
            "ckLINK".to_string(),
            Config::from_canister_id_str("g4tto-rqaaa-aaaar-qageq-cai"),
        ),
        (
            "ckUSDT".to_string(),
            Config::from_canister_id_str("cngnf-vqaaa-aaaar-qag4q-cai"),
        ),
        (
            "ckOCT".to_string(),
            Config::from_canister_id_str("ebo5g-cyaaa-aaaar-qagla-cai"),
        ),
        (
            "ckPEPE".to_string(),
            Config::from_canister_id_str("etik7-oiaaa-aaaar-qagia-cai"),
        ),
        (
            "ckXAUT".to_string(),
            Config::from_canister_id_str("nza5v-qaaaa-aaaar-qahzq-cai"),
        ),
        (
            "ckSHIB".to_string(),
            Config::from_canister_id_str("fxffn-xiaaa-aaaar-qagoa-cai"),
        ),
        (
            "ckWSTETH".to_string(),
            Config::from_canister_id_str("j2tuh-yqaaa-aaaar-qahcq-cai"),
        ),
        (
            "BOB".to_string(),
            Config::from_canister_id_str("7pail-xaaaa-aaaas-aabmq-cai"),
        ),
        (
            "AAA".to_string(),
            Config::from_canister_id_str("l67es-4iaaa-aaaag-atvda-cai"),
        ),
        (
            "ALEX".to_string(),
            Config::from_canister_id_str("ysy5f-2qaaa-aaaap-qkmmq-cai"),
        ),
        (
            "CLOWN".to_string(),
            Config::from_canister_id_str("iwv6l-6iaaa-aaaal-ajjjq-cai"),
        ),
        (
            "RUGGY".to_string(),
            Config::from_canister_id_str("icaf7-3aaaa-aaaam-qcx3q-cai"),
        ),
        (
            "GLDT".to_string(),
            Config::from_canister_id_str("6c7su-kiaaa-aaaar-qaira-cai"),
        ),
        (
            "EXE".to_string(),
            Config::from_canister_id_str("rh2pm-ryaaa-aaaan-qeniq-cai"),
        ),
        (
            "XTC".to_string(),
            Config::from_canister_id_str("aanaa-xaaaa-aaaah-aaeiq-cai"),
        ),
        (
            "SEER".to_string(),
            Config::from_canister_id_str("rffwt-piaaa-aaaaq-aabqq-cai"),
        ),
        (
            "CLOUD".to_string(),
            Config::from_canister_id_str("pcj6u-uaaaa-aaaak-aewnq-cai"),
        ),
        (
            "iDoge".to_string(),
            Config::from_canister_id_str("eayyd-iiaaa-aaaah-adtea-cai"),
        ),
        (
            "TAGGR".to_string(),
            Config::from_canister_id_str("6qfxa-ryaaa-aaaai-qbhsq-cai"),
        ),
    ];

    tokenlist.append(&mut get_token_key_value_other_ecosystem());

    tokenlist.into_iter().collect()
}

fn get_token_key_value_other_ecosystem() -> Vec<(String, Config)> {
    vec![
        (
            "BTC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BTC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ETH".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ETH".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "USDT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "USDT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "XRP".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "XRP".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BNB".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BNB".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SOL".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SOL".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "USDC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "USDC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "DOGE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "DOGE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ADA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ADA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "TRX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "TRX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SUI".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SUI".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "LINK".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "LINK".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "AVAX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "AVAX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BCH".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BCH".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "XLM".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "XLM".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "LEO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "LEO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SHIB".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SHIB".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "HBAR".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "HBAR".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "TON".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "TON".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "HYPE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "HYPE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "LTC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "LTC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "DOT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "DOT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "DAI".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "DAI".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "XMR".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "XMR".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BGB".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BGB".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "USDe".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "USDe".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "PI".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "PI".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "PEPE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "PEPE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "TAO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "TAO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "UNI".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "UNI".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "APT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "APT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "OKB".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "OKB".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "NEAR".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "NEAR".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ONDO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ONDO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "AAVE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "AAVE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "GT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "GT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ETC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ETC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ICP".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ICP".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "CRO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "CRO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "TRUMP".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "TRUMP".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "KAS".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "KAS".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "MNT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "MNT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "POL".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "POL".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "RENDER".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "RENDER".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "VET".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "VET".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "USD1".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "USD1".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "FIL".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "FIL".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ALGO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ALGO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ENA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ENA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "FET".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "FET".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ATOM".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ATOM".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "TIA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "TIA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ARB".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ARB".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "S".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "S".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "FDUSD".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "FDUSD".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "STX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "STX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BONK".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BONK".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "KCS".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "KCS".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "WLD".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "WLD".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "MKR".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "MKR".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "EOS".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "EOS".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "FLR".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "FLR".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "JUP".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "JUP".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "DEXE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "DEXE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "XDC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "XDC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "QNT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "QNT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "IP".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "IP".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "FARTCOIN".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "FARTCOIN".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SEI".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SEI".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "IMX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "IMX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "OP".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "OP".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "INJ".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "INJ".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "VIRTUAL".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "VIRTUAL".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "FORM".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "FORM".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "CRV".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "CRV".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "GRT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "GRT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "PYUSD".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "PYUSD".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "XAUt".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "XAUt".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "NEXO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "NEXO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "JASMY".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "JASMY".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "PAXG".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "PAXG".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "IOTA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "IOTA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "WAL".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "WAL".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "FLOKI".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "FLOKI".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "THETA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "THETA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BSV".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BSV".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "RAY".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "RAY".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "LDO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "LDO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "PENGU".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "PENGU".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SAND".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SAND".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "CORE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "CORE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "GALA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "GALA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BTT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BTT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ENS".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ENS".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "KAIA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "KAIA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "HNT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "HNT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "CAKE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "CAKE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ZEC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ZEC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ZKJ".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ZKJ".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "FLOW".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "FLOW".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "MANA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "MANA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "WIF".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "WIF".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BRETT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BRETT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "DEEP".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "DEEP".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "XCN".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "XCN".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "XTZ".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "XTZ".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "PENDLE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "PENDLE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "JTO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "JTO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "AERO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "AERO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SPX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SPX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "PYTH".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "PYTH".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "RSR".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "RSR".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "TUSD".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "TUSD".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "KAVA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "KAVA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "AIOZ".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "AIOZ".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "AR".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "AR".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "RUNE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "RUNE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "DYDX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "DYDX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "EGLD".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "EGLD".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "POPCAT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "POPCAT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "XEC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "XEC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ABAB".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ABAB".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "STRK".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "STRK".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "NFT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "NFT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "NEO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "NEO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SUPER".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SUPER".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "AXS".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "AXS".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "MOVE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "MOVE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "AKT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "AKT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "CHZ".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "CHZ".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BERA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BERA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "APE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "APE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BEAM".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BEAM".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "CFX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "CFX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "W".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "W".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "TURBO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "TURBO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "GRASS".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "GRASS".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "MORPHO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "MORPHO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "AXL".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "AXL".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "COMP".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "COMP".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "KAITO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "KAITO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "OM".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "OM".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SUN".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SUN".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "FTT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "FTT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "JST".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "JST".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "MOG".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "MOG".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "AMP".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "AMP".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "USDD".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "USDD".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SAFE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SAFE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "LUNC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "LUNC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "RON".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "RON".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "TWT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "TWT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "CVX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "CVX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "CTC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "CTC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "AI16Z".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "AI16Z".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "GNO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "GNO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "DOG".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "DOG".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "LAYER".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "LAYER".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ZRO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ZRO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "MINA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "MINA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "1INCH".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "1INCH".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "CHEEMS".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "CHEEMS".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ATH".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ATH".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "DASH".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "DASH".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SFP".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SFP".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "GLM".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "GLM".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "MEW".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "MEW".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "KSM".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "KSM".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "TFUEL".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "TFUEL".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "MX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "MX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ZIL".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ZIL".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SYRUP".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SYRUP".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "MOCA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "MOCA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BLUR".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BLUR".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "QTUM".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "QTUM".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ACH".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ACH".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "NOT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "NOT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "EIGEN".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "EIGEN".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SNX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SNX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "VTHO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "VTHO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "DCR".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "DCR".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ZRX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ZRX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "CKB".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "CKB".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ZETA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ZETA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BAT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BAT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BabyDoge".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BabyDoge".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ASTR".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ASTR".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "GAS".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "GAS".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "TRAC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "TRAC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ID".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ID".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ROSE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ROSE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ZK".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ZK".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "DRIFT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "DRIFT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "KDA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "KDA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "CELO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "CELO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "LPT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "LPT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BABY".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BABY".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "FRAX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "FRAX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ONE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ONE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ANKR".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ANKR".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "FTN".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "FTN".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "JLP".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "JLP".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ULTIMA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ULTIMA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "FLZ".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "FLZ".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "USD0".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "USD0".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "USDY".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "USDY".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ZBU".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ZBU".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "WOULD".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "WOULD".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "TEL".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "TEL".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BDX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BDX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "OHM".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "OHM".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "UPC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "UPC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "PLUME".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "PLUME".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "WHITE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "WHITE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "CHEEL".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "CHEEL".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "rLUSD".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "rLUSD".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "FRAX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "FRAX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "USDf".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "USDf".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SNEK".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SNEK".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "GHO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "GHO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "EURC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "EURC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "USDG".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "USDG".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BORG".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BORG".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "LGCT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "LGCT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "CHEX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "CHEX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ALE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ALE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "MELANIA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "MELANIA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "UXLINK".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "UXLINK".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "YFI".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "YFI".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "QUBIC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "QUBIC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SAROS".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SAROS".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "T".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "T".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "DEUSD".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "DEUSD".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ETHW".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ETHW".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "CSPR".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "CSPR".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "GIGA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "GIGA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ELF".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ELF".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "HOT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "HOT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SOS".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SOS".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "AIXBT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "AIXBT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "RVN".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "RVN".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "IOTX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "IOTX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "XYO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "XYO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SUSHI".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SUSHI".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "VANA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "VANA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "GOMINING".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "GOMINING".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "PNUT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "PNUT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SQD".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SQD".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "HMSTR".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "HMSTR".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "DGB".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "DGB".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "USDO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "USDO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "OSMO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "OSMO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "XCH".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "XCH".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "XEM".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "XEM".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "KOGE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "KOGE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "TOSHI".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "TOSHI".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "COTI".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "COTI".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "WEMIX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "WEMIX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "GMT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "GMT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "MPLX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "MPLX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ORDI".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ORDI".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "CETUS".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "CETUS".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "DLC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "DLC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ZANO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ZANO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "STPT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "STPT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ORCA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ORCA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ALCH".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ALCH".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ME".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ME".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ETHFI".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ETHFI".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "G".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "G".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "POLYX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "POLYX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "EUL".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "EUL".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "AIC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "AIC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "LRC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "LRC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "EURS".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "EURS".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "RLB".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "RLB".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ENJ".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ENJ".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "STIK".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "STIK".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "GMX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "GMX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SWFTC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SWFTC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ZEN".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ZEN".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "FAI".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "FAI".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BIGTIME".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BIGTIME".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ONT".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ONT".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "LCX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "LCX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "WAVES".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "WAVES".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "WOO".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "WOO".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "DAG".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "DAG".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "ZBCN".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ZBCN".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "DSYNC".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "DSYNC".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SKL".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SKL".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "SXP".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "SXP".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "GOHOME".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "GOHOME".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "FUSDF".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "FUSDF".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "BAND".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "BAND".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "COW".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "COW".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "LUNA".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "LUNA".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "HIVE".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "HIVE".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
        (
            "WMTX".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "WMTX".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::FiatCurrency,
                    symbol: "USD".to_string(),
                },
            },
        ),
    ]
}

/*
kongswap method:

kongswap api :https://github.com/KongSwap/kong/blob/main/src/kong_svelte/src/lib/api/index.ts

*/
