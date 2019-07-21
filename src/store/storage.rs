pub trait Storage<K, V> {
    fn set(&mut self, key: K, value: V);
    fn get(&self, key: &K) -> Option<V>;
    fn pull_out(&mut self, key: &K) -> Option<V>;
    fn remove(&mut self, key: &K);
}
