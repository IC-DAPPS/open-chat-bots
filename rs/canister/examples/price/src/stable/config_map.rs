use crate::memory::{get_memory, Memory};
use crate::price_provider::xrc::Asset;
use candid::{Decode, Encode, Principal};
use ic_stable_structures::memory_manager::MemoryId;
use ic_stable_structures::storable::{Bound, Storable};
use ic_stable_structures::StableBTreeMap;
use oc_bots_sdk::types::{BotActionChatDetails, BotCommandScope, CanisterId, ChannelId, Chat};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cell::RefCell;

const CONFIG_MAP_MEMORY_ID: MemoryId = MemoryId::new(1);

thread_local! {
    static MAP: RefCell<StableBTreeMap<ConfigKey, Config, Memory>> = RefCell::new(
            StableBTreeMap::init(
                get_memory(CONFIG_MAP_MEMORY_ID),
            )
    );
}

pub fn get(key: ConfigKey) -> Option<Config> {
    MAP.with(|p| p.borrow().get(&key))
}

pub fn insert(key: ConfigKey, value: Config) -> Option<Config> {
    MAP.with(|p| p.borrow_mut().insert(key, value))
}

pub fn remove(key: ConfigKey) -> Option<Config> {
    MAP.with(|p| p.borrow_mut().remove(&key))
}

pub fn contains_key(key: &ConfigKey) -> bool {
    MAP.with(|p| p.borrow().contains_key(key))
}

pub fn len() -> u64 {
    MAP.with(|p| p.borrow().len())
}

#[derive(candid::CandidType, Clone, Serialize, Debug, Deserialize)]
pub enum Config {
    XRC {
        base_asset: Asset,
        quote_asset: Asset,
    },
    ICPSwap {
        canister_id: Principal,
        // name: String,
    },
}

impl Config {
    pub fn xrc_asset_symbols(&self) -> Option<(&str, &str)> {
        match self {
            Config::XRC {
                base_asset,
                quote_asset,
            } => Some((&base_asset.symbol, &quote_asset.symbol)),
            _ => None,
        }
    }
}

impl Storable for Config {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

#[derive(
    candid::CandidType, Clone, Serialize, Debug, Deserialize, Eq, PartialEq, PartialOrd, Ord,
)]
pub enum ConfigKey {
    Direct(CanisterId),
    Group(CanisterId),
    Channel(CanisterId, ChannelId),
    Community(CanisterId),
}

impl Storable for ConfigKey {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

// #[derive(CandidType, Serialize, Deserialize, Clone, Debug)]

// TODO : Channel specific config is not implemented yet
impl ConfigKey {
    pub fn from_bot_cmd_scope(scope: BotCommandScope) -> Self {
        match scope {
            BotCommandScope::Chat(BotActionChatDetails { chat, .. }) => match chat {
                Chat::Channel(canister_id, _channel_id) => Self::Community(canister_id),
                Chat::Group(canister_id) => Self::Group(canister_id),
                Chat::Direct(canister_id) => Self::Direct(canister_id),
            },
            BotCommandScope::Community(bot_action_group_details) => {
                Self::Community(bot_action_group_details.community_id)
            }
        }
    }

    // pub fn new_direct(canister_id: CanisterId) -> Self {
    //     Self::Direct(canister_id)
    // }

    // pub fn new_group(canister_id: CanisterId) -> Self {
    //     Self::Group(canister_id)
    // }

    // pub fn new_channel(canister_id: CanisterId, channel_id: ChannelId) -> Self {
    //     Self::Channel(canister_id, channel_id)
    // }

    // pub fn new_community(canister_id: CanisterId) -> Self {
    //     Self::Community(canister_id)
    // }
}
