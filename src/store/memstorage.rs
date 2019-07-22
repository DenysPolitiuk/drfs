use std::cmp::Eq;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Mutex;

use super::Storage;

pub struct MemStorage<K, V> {
    map: Mutex<HashMap<K, V>>,
}

impl<K, V> MemStorage<K, V>
where
    K: Hash + Eq + Send + Sync,
    V: Send + Sync,
{
    pub fn new() -> MemStorage<K, V> {
        MemStorage {
            map: Mutex::new(HashMap::new()),
        }
    }
}

impl<K, V> Storage<K, V> for MemStorage<K, V>
where
    K: Hash + Eq + Send + Sync,
    V: Send + Sync + Clone,
{
    fn set(&self, key: K, value: V) {
        self.map.lock().unwrap().insert(key, value);
    }
    fn get(&self, key: &K) -> Option<V> {
        self.map.lock().unwrap().get(key).cloned()
    }
    fn pull_out(&self, key: &K) -> Option<V> {
        self.map.lock().unwrap().remove(key)
    }
    fn remove(&self, key: &K) {
        self.map.lock().unwrap().remove(key);
    }
}
