use core::borrow::Borrow;
use core::hash::Hash;

pub trait StorageFamily: Copy {
    type OrdMap<K: Ord + Eq + Clone, V: Clone>: OrdMapStorage<K, V>;
    type HashMap<K: Hash + Eq + Clone, V: Clone>: HashMapStorage<K, V>;
}

#[derive(Clone, Copy)]
pub struct ImArcStorage;

impl StorageFamily for ImArcStorage {
    type OrdMap<K: Ord + Eq + Clone, V: Clone> = im::OrdMap<K, V>;

    type HashMap<K: Hash + Eq + Clone, V: Clone> = im::HashMap<K, V>;
}

#[derive(Clone, Copy)]
pub struct ImRcStorage;

impl StorageFamily for ImRcStorage {
    type OrdMap<K: Ord + Eq + Clone, V: Clone> = im_rc::OrdMap<K, V>;

    type HashMap<K: Hash + Eq + Clone, V: Clone> = im_rc::HashMap<K, V>;
}

#[derive(Clone, Copy)]
pub struct StdStorage;

impl StorageFamily for StdStorage {
    type OrdMap<K: Ord + Eq + Clone, V: Clone> = std::collections::BTreeMap<K, V>;

    type HashMap<K: Hash + Eq + Clone, V: Clone> = std::collections::HashMap<K, V>;
}

pub trait OrdMapStorage<K: Ord + Eq + Clone, V: Clone>: Clone {
    fn new() -> Self;
    fn insert(&mut self, key: K, value: V);

    fn get<BK>(&self, k: &BK) -> Option<&V>
    where
        BK: Ord + ?Sized,
        K: Borrow<BK>;

    fn remove<BK>(&mut self, k: &BK) -> Option<V>
    where
        BK: Ord + ?Sized,
        K: Borrow<BK>;

    fn get_first(&self) -> Option<(&K, &V)>;
    fn pop_first(&mut self) -> Option<(K, V)>;
}

pub trait HashMapStorage<K: Hash + Eq + Clone, V: Clone>: Clone {
    fn new() -> Self;
    fn insert(&mut self, key: K, value: V);

    fn get<BK>(&self, k: &BK) -> Option<&V>
    where
        BK: Hash + Eq + ?Sized,
        K: Borrow<BK>;

    fn remove<BK>(&mut self, k: &BK) -> Option<V>
    where
        BK: Hash + Eq + ?Sized,
        K: Borrow<BK>;
}

impl<K: Ord + Eq + Clone, V: Clone> OrdMapStorage<K, V> for im::OrdMap<K, V> {
    fn new() -> Self {
        Self::new()
    }

    fn insert(&mut self, key: K, value: V) {
        let _ = self.insert(key, value);
    }

    fn get<BK>(&self, k: &BK) -> Option<&V>
    where
        BK: Ord + ?Sized,
        K: Borrow<BK>,
    {
        self.get(k)
    }

    fn remove<BK>(&mut self, k: &BK) -> Option<V>
    where
        BK: Ord + ?Sized,
        K: Borrow<BK>,
    {
        self.remove(k)
    }

    fn get_first(&self) -> Option<(&K, &V)> {
        self.get_min().map(|k_v| (&k_v.0, &k_v.1))
    }

    fn pop_first(&mut self) -> Option<(K, V)> {
        let (min, new) = self.without_min_with_key();
        *self = new;

        min
    }
}

impl<K: Hash + Eq + Clone, V: Clone> HashMapStorage<K, V> for im::HashMap<K, V> {
    fn new() -> Self {
        Self::new()
    }

    fn insert(&mut self, key: K, value: V) {
        let _ = self.insert(key, value);
    }

    fn get<BK>(&self, k: &BK) -> Option<&V>
    where
        BK: Hash + Eq + ?Sized,
        K: Borrow<BK>,
    {
        self.get(k)
    }

    fn remove<BK>(&mut self, k: &BK) -> Option<V>
    where
        BK: Hash + Eq + ?Sized,
        K: Borrow<BK>,
    {
        self.remove(k)
    }
}

impl<K: Ord + Eq + Clone, V: Clone> OrdMapStorage<K, V> for im_rc::OrdMap<K, V> {
    fn new() -> Self {
        Self::new()
    }

    fn insert(&mut self, key: K, value: V) {
        let _ = self.insert(key, value);
    }

    fn get<BK>(&self, k: &BK) -> Option<&V>
    where
        BK: Ord + ?Sized,
        K: Borrow<BK>,
    {
        self.get(k)
    }

    fn remove<BK>(&mut self, k: &BK) -> Option<V>
    where
        BK: Ord + ?Sized,
        K: Borrow<BK>,
    {
        self.remove(k)
    }

    fn get_first(&self) -> Option<(&K, &V)> {
        self.get_min().map(|k_v| (&k_v.0, &k_v.1))
    }

    fn pop_first(&mut self) -> Option<(K, V)> {
        let (min, new) = self.without_min_with_key();
        *self = new;

        min
    }
}

impl<K: Hash + Eq + Clone, V: Clone> HashMapStorage<K, V> for im_rc::HashMap<K, V> {
    fn new() -> Self {
        Self::new()
    }

    fn insert(&mut self, key: K, value: V) {
        let _ = self.insert(key, value);
    }

    fn get<BK>(&self, k: &BK) -> Option<&V>
    where
        BK: Hash + Eq + ?Sized,
        K: Borrow<BK>,
    {
        self.get(k)
    }

    fn remove<BK>(&mut self, k: &BK) -> Option<V>
    where
        BK: Hash + Eq + ?Sized,
        K: Borrow<BK>,
    {
        self.remove(k)
    }
}

impl<K: Ord + Eq + Clone, V: Clone> OrdMapStorage<K, V> for std::collections::BTreeMap<K, V> {
    fn new() -> Self {
        Self::new()
    }

    fn insert(&mut self, key: K, value: V) {
        let _ = self.insert(key, value);
    }

    fn get<BK>(&self, k: &BK) -> Option<&V>
    where
        BK: Ord + ?Sized,
        K: Borrow<BK>,
    {
        self.get(k)
    }

    fn remove<BK>(&mut self, k: &BK) -> Option<V>
    where
        BK: Ord + ?Sized,
        K: Borrow<BK>,
    {
        self.remove(k)
    }

    fn get_first(&self) -> Option<(&K, &V)> {
        self.first_key_value()
    }

    fn pop_first(&mut self) -> Option<(K, V)> {
        self.pop_first()
    }
}

impl<K: Hash + Eq + Clone, V: Clone> HashMapStorage<K, V> for std::collections::HashMap<K, V> {
    fn new() -> Self {
        Self::new()
    }

    fn insert(&mut self, key: K, value: V) {
        let _ = self.insert(key, value);
    }

    fn get<BK>(&self, k: &BK) -> Option<&V>
    where
        BK: Hash + Eq + ?Sized,
        K: Borrow<BK>,
    {
        self.get(k)
    }

    fn remove<BK>(&mut self, k: &BK) -> Option<V>
    where
        BK: Hash + Eq + ?Sized,
        K: Borrow<BK>,
    {
        self.remove(k)
    }
}
