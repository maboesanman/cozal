use std::collections::hash_map::{
    HashMap,
    RandomState,
    RawEntryMut,
    RawOccupiedEntryMut,
    RawVacantEntryMut,
};
use std::hash::{BuildHasher, Hash};
use std::ptr::NonNull;

pub fn get_occupied<K, V, S>(
    this: &mut HashMap<K, V, S>,
    key: K,
) -> Result<OccupiedExtEntry<'_, K, V, S>, VacantExtEntry<'_, K, V, S>>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    let hash_map: NonNull<_> = this.into();
    let builder = this.raw_entry_mut();

    let entry = builder.from_key(&key);

    match entry {
        RawEntryMut::Occupied(entry) => Ok(OccupiedExtEntry {
            hash_map,
            entry,
        }),
        RawEntryMut::Vacant(entry) => Err(VacantExtEntry {
            hash_map,
            entry,
            key,
        }),
    }
}

#[derive(Debug)]
pub struct OccupiedExtEntry<'a, K, V, S = RandomState> {
    hash_map: NonNull<HashMap<K, V, S>>,
    entry:    RawOccupiedEntryMut<'a, K, V, S>,
}

impl<'a, K: Hash + Eq, V, S: BuildHasher> OccupiedExtEntry<'a, K, V, S> {
    pub fn get_key(&self) -> &K {
        self.entry.key()
    }

    pub fn into_key(self) -> &'a K {
        self.entry.into_key()
    }

    pub fn get_value(&self) -> &V {
        self.entry.get()
    }

    pub fn get_value_mut(&mut self) -> &mut V {
        self.entry.get_mut()
    }

    pub fn into_value_mut(self) -> &'a mut V {
        self.entry.into_mut()
    }

    pub fn get_key_value(&self) -> (&K, &V) {
        let k = self.entry.key();
        let v = self.entry.get();
        (k, v)
    }

    pub fn get_key_value_mut(&mut self) -> (&K, &mut V) {
        let (k, v) = self.entry.get_key_value_mut();
        (k, v)
    }

    pub fn into_key_value(self) -> (&'a K, &'a mut V) {
        let (k, v) = self.entry.into_key_value();
        (k, v)
    }

    pub fn vacate(self) -> (VacantExtEntry<'a, K, V, S>, V) {
        let Self {
            entry,
            mut hash_map,
        } = self;

        let (key, value) = entry.remove_entry();

        // SAFETY: this is kept alive by the lifetime 'a,
        // and does not alias entry because it's dropped by remove_entry.
        let hash_map_mut = unsafe { hash_map.as_mut() };

        let builder = hash_map_mut.raw_entry_mut();
        let entry = builder.from_key(&key);
        let entry = match entry {
            RawEntryMut::Occupied(_) => unreachable!(),
            RawEntryMut::Vacant(v) => v,
        };

        let new_entry = VacantExtEntry {
            hash_map,
            entry,
            key,
        };

        (new_entry, value)
    }

    pub fn into_collection_mut(self) -> &'a mut HashMap<K, V, S> {
        let Self {
            entry: _,
            mut hash_map,
        } = self;

        // SAFETY: this is kept alive by the lifetime 'a,
        // and does not alias entry because it's dropped.
        unsafe { hash_map.as_mut() }
    }

    pub fn get_collection_ref(&self) -> &HashMap<K, V, S> {
        unsafe { self.hash_map.as_ref() }
    }
}

#[derive(Debug)]
pub struct VacantExtEntry<'a, K, V, S = RandomState> {
    hash_map: NonNull<HashMap<K, V, S>>,
    entry:    RawVacantEntryMut<'a, K, V, S>,
    key:      K,
}

impl<'a, K: Hash + Eq, V, S: BuildHasher> VacantExtEntry<'a, K, V, S> {
    pub fn get_key(&self) -> &K {
        &self.key
    }

    pub fn into_key(self) -> K {
        self.key
    }

    pub fn occupy(self, value: V) -> OccupiedExtEntry<'a, K, V, S> {
        let Self {
            mut hash_map,
            entry,
            key,
        } = self;

        let (k, _) = entry.insert(key, value);

        // SAFETY: this is kept alive by the lifetime 'a,
        // and does not alias entry because it's dropped by remove_entry.
        let hash_map_mut = unsafe { hash_map.as_mut() };

        let builder = hash_map_mut.raw_entry_mut();
        let entry = builder.from_key(k);
        let entry = match entry {
            RawEntryMut::Occupied(o) => o,
            RawEntryMut::Vacant(_) => unreachable!(),
        };

        OccupiedExtEntry {
            hash_map,
            entry,
        }
    }

    pub fn into_collection_mut(self) -> (&'a mut HashMap<K, V, S>, K) {
        let Self {
            mut hash_map,
            entry: _,
            key,
        } = self;

        // SAFETY: this is kept alive by the lifetime 'a,
        // and does not alias entry because it's dropped.
        (unsafe { hash_map.as_mut() }, key)
    }

    pub fn get_collection_ref(&self) -> &HashMap<K, V, S> {
        unsafe { self.hash_map.as_ref() }
    }
}
