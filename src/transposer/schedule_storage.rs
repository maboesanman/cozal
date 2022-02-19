use core::borrow::Borrow;
use core::hash::Hash;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

pub trait StorageFamily: Copy {
    type OrdMap<K: Ord + Eq + Clone, V: Clone>: OrdMapStorage<K, V>;
    type HashMap<K: Hash + Eq + Clone, V: Clone>: HashMapStorage<K, V>;

    // someday we want to drop this clone bound and specialize the Arc impl when W is clone.
    type Transposer<W: Clone>: TransposerPointer<W>;
}

#[derive(Clone, Copy)]
pub struct DefaultStorage;

impl StorageFamily for DefaultStorage {
    type OrdMap<K: Ord + Eq + Clone, V: Clone> = im::OrdMap<K, V>;
    type HashMap<K: Hash + Eq + Clone, V: Clone> = im::HashMap<K, V>;
    type Transposer<W: Clone> = Box<W>;
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

pub trait TransposerPointer<T>: Deref<Target = T> + Unpin {
    type Borrowed<'a>: Deref<Target = T>
    where
        Self: 'a,
        T: 'a;

    fn new(inner: T) -> Self;

    fn borrow(&self) -> Self::Borrowed<'_>;
    fn mutate(&mut self) -> &mut T;

    fn try_take(self) -> Option<T>;
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

impl<T> TransposerPointer<T> for Box<T> {
    type Borrowed<'a>
    where
        Self: 'a,
        T: 'a,
    = &'a T;

    fn new(inner: T) -> Self {
        Box::new(inner)
    }

    fn borrow(&self) -> Self::Borrowed<'_> {
        self
    }

    fn mutate(&mut self) -> &mut T {
        self
    }

    fn try_take(self) -> Option<T> {
        Some(*self)
    }
}

impl<T: Clone> TransposerPointer<T> for Arc<T> {
    type Borrowed<'a>
    where
        T: 'a,
    = Arc<T>;

    fn new(inner: T) -> Self {
        Arc::new(inner)
    }

    fn borrow(&self) -> Self::Borrowed<'_> {
        self.clone()
    }

    fn mutate(&mut self) -> &mut T {
        Arc::make_mut(self)
    }

    fn try_take(self) -> Option<T> {
        Arc::try_unwrap(self).ok()
    }
}

impl<T: Clone> TransposerPointer<T> for Rc<T> {
    type Borrowed<'a>
    where
        T: 'a,
    = Rc<T>;

    fn new(inner: T) -> Self {
        Rc::new(inner)
    }

    fn borrow(&self) -> Self::Borrowed<'_> {
        self.clone()
    }

    fn mutate(&mut self) -> &mut T {
        Rc::make_mut(self)
    }

    fn try_take(self) -> Option<T> {
        Rc::try_unwrap(self).ok()
    }
}
