use crate::memory::{get_memory, Memory};
use candid::{Decode, Encode};
use ic_stable_structures::memory_manager::MemoryId;
use ic_stable_structures::storable::{Bound, Storable};
use ic_stable_structures::StableBTreeMap;
use oc_bots_sdk::types::{BotActionChatDetails, BotCommandScope, CanisterId, ChannelId, Chat};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cell::RefCell;

const FAQ_MAP_MEMORY_ID: MemoryId = MemoryId::new(1);

thread_local! {
    static MAP: RefCell<StableBTreeMap<Key, String, Memory>> = RefCell::new(
            StableBTreeMap::init(
                get_memory(FAQ_MAP_MEMORY_ID),
            )
    );
}

pub fn get(key: &Key) -> Option<String> {
    MAP.with(|p| p.borrow().get(key))
}

pub fn insert(key: Key, value: String) -> Option<String> {
    MAP.with(|p| p.borrow_mut().insert(key, value))
}

pub fn remove(key: Key) -> Option<String> {
    MAP.with(|p| p.borrow_mut().remove(&key))
}

pub fn contains_key(key: &Key) -> bool {
    MAP.with(|p| p.borrow().contains_key(key))
}

pub fn len() -> u64 {
    MAP.with(|p| p.borrow().len())
}

// impl Storable for Config {
//     const BOUND: Bound = Bound::Unbounded;

//     fn to_bytes(&self) -> Cow<[u8]> {
//         Cow::Owned(Encode!(self).unwrap())
//     }

//     fn from_bytes(bytes: Cow<[u8]>) -> Self {
//         Decode!(bytes.as_ref(), Self).unwrap()
//     }
// }

#[derive(
    candid::CandidType, Clone, Serialize, Debug, Deserialize, Eq, PartialEq, PartialOrd, Ord,
)]
pub enum Key {
    // Direct(CanisterId),
    Group(CanisterId),
    Channel(CanisterId, ChannelId),
    Community(CanisterId),
}

impl Storable for Key {
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
impl Key {
    pub fn from_bot_cmd_scope(scope: BotCommandScope) -> Result<Self, String> {
        match scope {
            BotCommandScope::Chat(BotActionChatDetails { chat, .. }) => match chat {
                Chat::Channel(canister_id, _channel_id) => Ok(Self::Community(canister_id)),
                Chat::Group(canister_id) => Ok(Self::Group(canister_id)),
                Chat::Direct(_) =>Err("FAQ functionality isn't available in direct chats. FAQBot is intended for communities and groups.".to_string())
            },
            BotCommandScope::Community(bot_action_group_details) => {
                Ok(Self::Community(bot_action_group_details.community_id))
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
