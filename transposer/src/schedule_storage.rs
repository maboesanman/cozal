use core::borrow::Borrow;
use core::hash::Hash;
use core::ops::Deref;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::Arc;

pub trait StorageFamily: Copy + 'static {
    type OrdMap<K: Ord + Eq + Clone, V: Clone>: OrdMapStorage<K, V>;
    type HashMap<K: Hash + Eq + Clone, V: Clone>: HashMapStorage<K, V>;

    // someday we want to drop this clone bound and specialize the Arc impl when W is clone.
    type Transposer<W>: RefCounted<W>;

    type LazyState<W: ?Sized>: RefCounted<W> + Clone;
}

#[derive(Clone, Copy)]
pub struct DefaultStorage;

impl StorageFamily for DefaultStorage {
    type OrdMap<K: Ord + Eq + Clone, V: Clone> = im::OrdMap<K, V>;
    type HashMap<K: Hash + Eq + Clone, V: Clone> = im::HashMap<K, V>;
    type Transposer<W> = Arc<W>;
    type LazyState<W: ?Sized> = Arc<W>;
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

pub trait RefCounted<T: ?Sized>: Deref<Target = T> + Unpin {
    type Borrowed: Deref<Target = T> + Unpin;

    fn new(inner: Box<T>) -> Self;

    fn borrow(&self) -> Self::Borrowed;
    fn mutate(&mut self) -> &mut T
    where
        T: Clone;

    fn try_take(self) -> Option<T>
    where
        T: Sized;
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

impl<T: ?Sized> RefCounted<T> for Arc<T> {
    type Borrowed = Arc<T>;

    fn new(inner: Box<T>) -> Self {
        Arc::from(inner)
    }

    fn borrow(&self) -> Self::Borrowed {
        self.clone()
    }

    fn mutate(&mut self) -> &mut T
    where
        T: Clone,
    {
        Arc::make_mut(self)
    }

    fn try_take(self) -> Option<T>
    where
        T: Sized,
    {
        Arc::try_unwrap(self).ok()
    }
}

impl<T: ?Sized> RefCounted<T> for Rc<T> {
    type Borrowed = Rc<T>;

    fn new(inner: Box<T>) -> Self {
        Rc::from(inner)
    }

    fn borrow(&self) -> Self::Borrowed {
        self.clone()
    }

    fn mutate(&mut self) -> &mut T
    where
        T: Clone,
    {
        Rc::make_mut(self)
    }

    fn try_take(self) -> Option<T>
    where
        T: Sized,
    {
        Rc::try_unwrap(self).ok()
    }
}
