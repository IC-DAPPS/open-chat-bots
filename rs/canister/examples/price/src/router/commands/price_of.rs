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
    vec![
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
        BotCommandOptionChoice {
            name: "ckUSDC".to_string(),
            value: "ckUSDC".to_string(),
        },
        BotCommandOptionChoice {
            name: "ckETH".to_string(),
            value: "ckETH".to_string(),
        },
        BotCommandOptionChoice {
            name: "ckBTC".to_string(),
            value: "ckBTC".to_string(),
        },
        BotCommandOptionChoice {
            name: "ckLINK".to_string(),
            value: "ckLINK".to_string(),
        },
        BotCommandOptionChoice {
            name: "ckUSDT".to_string(),
            value: "ckUSDT".to_string(),
        },
        BotCommandOptionChoice {
            name: "ckOCT".to_string(),
            value: "ckOCT".to_string(),
        },
        BotCommandOptionChoice {
            name: "ckPEPE".to_string(),
            value: "ckPEPE".to_string(),
        },
        BotCommandOptionChoice {
            name: "ckXAUT".to_string(),
            value: "ckXAUT".to_string(),
        },
        BotCommandOptionChoice {
            name: "ckSHIB".to_string(),
            value: "ckSHIB".to_string(),
        },
        BotCommandOptionChoice {
            name: "ckWSTETH".to_string(),
            value: "ckWSTETH".to_string(),
        },
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
    ]
}

fn get_tokens_map() -> HashMap<String, Config> {
    let tokenlist: Vec<(String, Config)> = vec![
        (
            "ICP_xrc".to_string(),
            Config::XRC {
                base_asset: Asset {
                    class: AssetClass::Cryptocurrency,
                    symbol: "ICP".to_string(),
                },
                quote_asset: Asset {
                    class: AssetClass::Cryptocurrency,
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

    tokenlist.into_iter().collect()
}

/*
kongswap method:

kongswap api :https://github.com/KongSwap/kong/blob/main/src/kong_svelte/src/lib/api/index.ts

*/
