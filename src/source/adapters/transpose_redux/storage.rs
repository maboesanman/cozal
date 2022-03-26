use core::hash::Hash;
use std::rc::Rc;
use std::sync::Arc;

use crate::transposer::schedule_storage::StorageFamily;

#[derive(Clone, Copy)]
pub struct TransposeStorage;

impl StorageFamily for TransposeStorage {
    type OrdMap<K: Ord + Eq + Clone, V: Clone> = im_rc::OrdMap<K, V>;
    type HashMap<K: Hash + Eq + Clone, V: Clone> = im_rc::HashMap<K, V>;
    type Transposer<T: Clone> = Rc<T>;
    type LazyState<T> = Rc<T>;
}

// this is never actually instantiated. it is used in a where clause for a Send impl.
#[derive(Clone, Copy)]
pub struct DummySendStorage;

impl StorageFamily for DummySendStorage {
    type OrdMap<K: Ord + Eq + Clone, V: Clone> = im::OrdMap<K, V>;
    type HashMap<K: Hash + Eq + Clone, V: Clone> = im::HashMap<K, V>;
    type Transposer<T: Clone> = Arc<T>;
    type LazyState<T> = Arc<T>;
}
