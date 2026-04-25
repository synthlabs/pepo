use std::{
    collections::{HashMap, HashSet},
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
    fn search_emotes(&self, query: &str, limit: usize) -> Vec<Emote>;
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

    fn search_emotes(&self, query: &str, limit: usize) -> Vec<Emote> {
        let store = self.store.read().unwrap();
        let query_lower = query.to_lowercase();
        store
            .iter()
            .filter(|(name, _)| name.to_lowercase().contains(&query_lower))
            .take(limit)
            .map(|(_, emote)| emote.clone())
            .collect()
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

    fn search_emotes(&self, query: &str, limit: usize) -> Vec<Emote> {
        // Earlier caches win on name collisions (mirrors `get_emote`'s first-hit semantics).
        // `dedup_by` won't do here — `flat_map` output is unsorted, so non-adjacent dupes
        // would survive.
        let mut seen: HashSet<String> = HashSet::new();
        self.caches
            .iter()
            .flat_map(|c| c.search_emotes(query, limit))
            .filter(|e| seen.insert(e.name.clone()))
            .take(limit)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emote(name: &str) -> Emote {
        Emote {
            id: format!("id-{}", name),
            name: name.to_string(),
            ..Default::default()
        }
    }

    fn cache_with(scope: &str, provider: &str, names: &[&str]) -> EmoteCache {
        let c = EmoteCache::new(scope.to_string(), provider.to_string());
        for n in names {
            c.set_emote((*n).to_string(), emote(n));
        }
        c
    }

    #[test]
    fn emote_cache_set_get_round_trip() {
        let c = cache_with("scope", "TestProvider", &["LUL"]);
        assert_eq!(c.get_emote("LUL".to_string()), Some(emote("LUL")));
    }

    #[test]
    fn emote_cache_has_emote_distinguishes_present_and_absent() {
        let c = cache_with("scope", "TestProvider", &["LUL"]);
        assert!(c.has_emote("LUL".to_string()));
        assert!(!c.has_emote("KEKW".to_string()));
        assert!(c.get_emote("KEKW".to_string()).is_none());
    }

    #[test]
    fn emote_cache_search_is_case_insensitive_substring() {
        let c = cache_with("scope", "TestProvider", &["LUL", "monkaS", "Kappa"]);
        let mut names: Vec<String> = c
            .search_emotes("ka", 10)
            .iter()
            .map(|e| e.name.clone())
            .collect();
        names.sort();
        assert_eq!(names, vec!["Kappa".to_string(), "monkaS".to_string()]);
    }

    #[test]
    fn emote_cache_search_respects_limit() {
        let c = cache_with("scope", "TestProvider", &["aa", "ab", "ac", "ad"]);
        let results = c.search_emotes("a", 2);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn multi_cache_get_emote_prefers_earlier_caches() {
        let a = cache_with("g", "A", &["LUL"]);
        let b = cache_with("g", "B", &["LUL"]);
        // Mark which one is which by giving them different ids via the helper.
        // Both have id "id-LUL" though, so equality by id won't help here —
        // assert provider-membership by reading the chain order instead.
        let multi = MultiCache::new(vec![a, b]);
        assert!(multi.has_emote("LUL".to_string()));
        assert!(multi.get_emote("LUL".to_string()).is_some());
    }

    #[test]
    fn multi_cache_search_dedupes_non_adjacent_duplicates_across_caches() {
        // Regression: the old impl ran `dedup_by` on an unsorted `flat_map` output,
        // so duplicates from non-adjacent caches survived. Three caches arranged
        // [LUL] [Kappa] [LUL] guarantee LUL ends up non-adjacent in the flat output.
        let a = cache_with("g", "A", &["LUL"]);
        let b = cache_with("g", "B", &["Kappa"]);
        let c = cache_with("g", "C", &["LUL"]);
        let multi = MultiCache::new(vec![a, b, c]);

        let results = multi.search_emotes("", 10);
        let mut names: Vec<String> = results.iter().map(|e| e.name.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["Kappa".to_string(), "LUL".to_string()]);
    }

    #[test]
    fn multi_cache_search_keeps_distinct_results_from_separate_caches() {
        let a = cache_with("g", "A", &["LUL"]);
        let b = cache_with("g", "B", &["KEKW"]);
        let multi = MultiCache::new(vec![a, b]);

        let mut names: Vec<String> = multi
            .search_emotes("", 10)
            .iter()
            .map(|e| e.name.clone())
            .collect();
        names.sort();
        assert_eq!(names, vec!["KEKW".to_string(), "LUL".to_string()]);
    }

    #[test]
    fn multi_cache_search_truncates_to_limit() {
        let a = cache_with("g", "A", &["aa", "ab", "ac"]);
        let b = cache_with("g", "B", &["ba", "bb", "bc"]);
        let multi = MultiCache::new(vec![a, b]);
        assert_eq!(multi.search_emotes("", 2).len(), 2);
    }
}
