use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use tracing::debug;

use crate::emote::Emote;

#[derive(Clone)]
pub struct EmoteCache {
    pub name: String,
    store: Arc<RwLock<HashMap<String, Emote>>>,
}

pub trait EmoteCacheTrait {
    fn name(self) -> String;
    fn set_emote(self, name: String, emote: Emote);
    fn get_emote(self, name: String) -> Option<Emote>;
}

impl EmoteCache {
    pub fn new(name: String) -> Self {
        EmoteCache {
            name,
            store: Default::default(),
        }
    }
}

impl EmoteCacheTrait for EmoteCache {
    fn name(self) -> String {
        self.name.clone()
    }

    fn set_emote(self, name: String, emote: Emote) {
        let mut store = self.store.write().unwrap();
        debug!(
            name,
            "set_emote: cache_size={}, emote={:?}",
            store.len(),
            emote
        );
        store.insert(name, emote);
    }

    fn get_emote(self, name: String) -> Option<Emote> {
        let store = self.store.read().unwrap();
        debug!(name, "get_emote: cache_size={}", store.len());
        store.get(&name).cloned()
    }
}
