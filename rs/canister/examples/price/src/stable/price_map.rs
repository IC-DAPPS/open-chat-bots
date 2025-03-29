use crate::memory::{get_memory, Memory};
use crate::price_provider::xrc::Asset;
use candid::{Decode, Encode, Principal};
use ic_stable_structures::memory_manager::MemoryId;
use ic_stable_structures::storable::{Bound, Storable};
use ic_stable_structures::StableBTreeMap;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cell::RefCell;

use super::config_map::Config;

const PRICE_MEMORY_ID: MemoryId = MemoryId::new(2);

thread_local! {
    static MAP: RefCell<StableBTreeMap<String, PriceStore, Memory>> = RefCell::new(
            StableBTreeMap::init(
                get_memory(PRICE_MEMORY_ID),
            )
    );
}

pub fn get(key: String) -> Option<PriceStore> {
    MAP.with(|p| p.borrow().get(&key))
}

pub fn insert(key: String, value: PriceStore) -> Option<PriceStore> {
    MAP.with(|p| p.borrow_mut().insert(key, value))
}

pub fn remove(key: String) -> Option<PriceStore> {
    MAP.with(|p| p.borrow_mut().remove(&key))
}

pub fn contains_key(key: &String) -> bool {
    MAP.with(|p| p.borrow().contains_key(key))
}

pub fn len() -> u64 {
    MAP.with(|p| p.borrow().len())
}

#[derive(candid::CandidType, Clone, Serialize, Debug, Deserialize)]
pub struct PriceStore {
    pub price: f64,
    pub expiration_time: u64,
    pub name: Option<String>,
}

impl Storable for PriceStore {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

pub fn price_key_from_ledgerid(canister_id: Principal) -> String {
    canister_id.to_string()
}

pub fn price_key_from_base_quote_asset(base: &Asset, quote: &Asset) -> String {
    format!(
        "{}/{}[{}/{}]",
        base.symbol,
        quote.symbol,
        base.class.to_string(),
        quote.class.to_string()
    )
}

pub fn price_key_from_config(config: Config) -> String {
    match config {
        Config::ICPSwap {
            canister_id, /* name */
        } => price_key_from_ledgerid(canister_id),
        Config::XRC {
            base_asset,
            quote_asset,
        } => price_key_from_base_quote_asset(&base_asset, &quote_asset),
    }
}
