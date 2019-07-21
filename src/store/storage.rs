pub trait Storage<K, V>
where
    K: Send + Sync,
    V: Send + Sync,
{
    fn set(&self, key: K, value: V);
    fn get(&self, key: &K) -> Option<V>;
    fn pull_out(&self, key: &K) -> Option<V>;
    fn remove(&self, key: &K);
}
