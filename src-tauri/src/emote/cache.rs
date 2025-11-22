use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use tracing::debug;

use crate::emote::Emote;

#[derive(Clone)]
pub struct EmoteCache {
    pub scope: String,
    pub provider: String,
    store: Arc<RwLock<HashMap<String, Emote>>>,
}

pub trait EmoteCacheTrait {
    fn name(&self) -> String;
    fn set_emote(self, name: String, emote: Emote);
    fn get_emote(self, name: String) -> Option<Emote>;
}

impl EmoteCache {
    pub fn new(scope: String, provider: String) -> Self {
        EmoteCache {
            scope,
            provider,
            store: Default::default(),
        }
    }
}

impl EmoteCacheTrait for EmoteCache {
    fn name(&self) -> String {
        format!("{}:{}", self.provider.clone(), self.scope.clone())
    }

    fn set_emote(self, name: String, emote: Emote) {
        let mut store = self.store.write().unwrap();
        debug!(
            scope = self.scope.clone(),
            name = self.name(),
            cache_size = store.len(),
            "set_emote: emote={:?}",
            emote
        );
        store.insert(name, emote);
    }

    fn get_emote(self, name: String) -> Option<Emote> {
        let store = self.store.read().unwrap();
        debug!(
            scope = self.scope.clone(),
            name = self.name(),
            cache_size = store.len(),
            "get_emote"
        );
        store.get(&name).cloned()
    }
}
