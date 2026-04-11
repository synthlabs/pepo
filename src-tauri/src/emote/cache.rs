use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use tracing::{debug, trace};

use crate::emote::Emote;

#[derive(Clone)]
pub struct EmoteCache {
    pub scope: String,
    pub provider: String,
    store: Arc<RwLock<HashMap<String, Emote>>>,
}

pub trait EmoteCacheTrait {
    fn name(&self) -> String;
    fn providers(&self) -> Vec<String>;
    fn set_emote(&self, name: String, emote: Emote);
    fn get_emote(&self, name: String) -> Option<Emote>;
    fn has_emote(&self, name: String) -> bool;
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

    fn set_emote(&self, name: String, emote: Emote) {
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

    fn get_emote(&self, name: String) -> Option<Emote> {
        let store = self.store.read().unwrap();
        trace!(
            scope = self.name(),
            name = name.clone(),
            cache_size = store.len(),
            "get_emote"
        );
        store.get(&name).cloned()
    }

    fn providers(&self) -> Vec<String> {
        vec![self.provider.clone()]
    }

    fn has_emote(&self, name: String) -> bool {
        let store = self.store.read().unwrap();
        trace!(
            scope = self.name(),
            name = name.clone(),
            cache_size = store.len(),
            "has_emote"
        );

        store.contains_key(&name)
    }
}

/// A cache that checks an ordered list of caches, returning the first match.
pub struct MultiCache {
    caches: Vec<EmoteCache>,
}

impl MultiCache {
    pub fn new(caches: Vec<EmoteCache>) -> Self {
        MultiCache { caches }
    }

    pub fn into_caches(self) -> Vec<EmoteCache> {
        self.caches
    }
}

impl EmoteCacheTrait for MultiCache {
    fn name(&self) -> String {
        let names: Vec<String> = self.caches.iter().map(|c| c.name()).collect();
        format!("multi:[{}]", names.join(", "))
    }

    fn providers(&self) -> Vec<String> {
        self.caches.iter().flat_map(|c| c.providers()).collect()
    }

    fn set_emote(&self, name: String, emote: Emote) {
        if let Some(first) = self.caches.first() {
            first.set_emote(name, emote);
        }
    }

    fn get_emote(&self, name: String) -> Option<Emote> {
        for cache in &self.caches {
            if let Some(emote) = cache.get_emote(name.clone()) {
                return Some(emote);
            }
        }
        None
    }

    fn has_emote(&self, name: String) -> bool {
        self.caches.iter().any(|c| c.has_emote(name.clone()))
    }
}
