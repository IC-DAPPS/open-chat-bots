use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    DefaultMemoryImpl,
};
use std::cell::RefCell;

const UPGRADES: MemoryId = MemoryId::new(0);
// const CONFIG_MAP_MEMORY_ID: MemoryId = MemoryId::new(1);
// const PRICE_MEMORY_ID: MemoryId = MemoryId::new(2);

pub type Memory = VirtualMemory<DefaultMemoryImpl>;

thread_local! {
    // static MEMORY_MANAGER: MemoryManager<DefaultMemoryImpl>
    //     = MemoryManager::init_with_bucket_size(DefaultMemoryImpl::default(), 128);

    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
    RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));


    // // Initialize a `StableBTreeMap` with `MemoryId(0)`.
    // static MAP: RefCell<StableBTreeMap<u128, u128, Memory>> = RefCell::new(
    //     StableBTreeMap::init(
    //         MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))),
    //     )
    // );

}

pub fn get_upgrades_memory() -> Memory {
    get_memory(UPGRADES)
}

pub fn get_memory(id: MemoryId) -> Memory {
    MEMORY_MANAGER.with(|m| m.borrow().get(id))
}
