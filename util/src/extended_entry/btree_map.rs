use std::collections::btree_map::{self, BTreeMap, Entry, OccupiedEntry, VacantEntry};
use std::ptr::NonNull;

pub fn get_occupied<K, V>(
    collection: &mut BTreeMap<K, V>,
    key: K,
) -> Result<OccupiedExtEntry<'_, K, V>, VacantExtEntry<'_, K, V>>
where
    K: Ord,
{
    let btree_map: NonNull<_> = collection.into();
    let entry = collection.entry(key);

    match entry {
        Entry::Occupied(entry) => Ok(OccupiedExtEntry {
            btree_map,
            entry,
        }),
        Entry::Vacant(entry) => Err(VacantExtEntry {
            btree_map,
            entry,
        }),
    }
}

pub fn get_first_occupied_mut<K: Ord, V>(
    collection: &mut BTreeMap<K, V>,
) -> Option<OccupiedExtEntry<'_, K, V>> {
    let btree_map: NonNull<_> = collection.into();
    let entry = collection.first_entry()?;

    let entry = OccupiedExtEntry {
        btree_map,
        entry,
    };

    Some(entry)
}

#[derive(Debug)]
pub struct OccupiedExtEntry<'a, K: Ord, V> {
    btree_map: NonNull<BTreeMap<K, V>>,
    entry:     OccupiedEntry<'a, K, V>,
}

impl<'a, K: Ord, V> OccupiedExtEntry<'a, K, V> {
    pub fn get_key(&self) -> &K {
        self.entry.key()
    }

    pub fn into_key(self) -> &'a K {
        unimplemented!()
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
        unimplemented!()
    }

    pub fn into_key_value(self) -> (&'a K, &'a mut V) {
        unimplemented!()
    }

    pub fn vacate(self) -> (VacantExtEntry<'a, K, V>, V) {
        let Self {
            entry,
            mut btree_map,
        } = self;

        let (key, value) = entry.remove_entry();

        // SAFETY: this is kept alive by the lifetime 'a,
        // and does not alias entry because it's dropped by remove_entry.
        let btree_map_mut = unsafe { btree_map.as_mut() };

        let entry = btree_map_mut.entry(key);
        let entry = match entry {
            Entry::Occupied(_) => unreachable!(),
            Entry::Vacant(v) => v,
        };

        let new_entry = VacantExtEntry {
            btree_map,
            entry,
        };

        (new_entry, value)
    }

    pub fn into_collection_mut(self) -> &'a mut BTreeMap<K, V> {
        let Self {
            entry: _,
            mut btree_map,
        } = self;

        // SAFETY: this is kept alive by the lifetime 'a,
        // and does not alias entry because it's dropped.
        unsafe { btree_map.as_mut() }
    }

    pub fn get_collection_ref(&self) -> &BTreeMap<K, V> {
        unsafe { self.btree_map.as_ref() }
    }
}

#[derive(Debug)]
pub struct VacantExtEntry<'a, K: Ord, V> {
    btree_map: NonNull<BTreeMap<K, V>>,
    entry:     VacantEntry<'a, K, V>,
}

impl<'a, K: Ord, V> VacantExtEntry<'a, K, V> {
    pub fn get_key(&self) -> &K {
        self.entry.key()
    }

    pub fn into_key(self) -> K {
        self.entry.into_key()
    }

    pub fn occupy(self, value: V) -> OccupiedExtEntry<'a, K, V>
    where
        K: Clone,
    {
        let Self {
            mut btree_map,
            entry,
        } = self;

        let key = entry.key().clone();

        entry.insert(value);

        // SAFETY: this is kept alive by the lifetime 'a,
        // and does not alias entry because it's dropped by remove_entry.
        let btree_map_mut = unsafe { btree_map.as_mut() };

        let entry = btree_map_mut.entry(key);
        let entry = match entry {
            Entry::Occupied(o) => o,
            Entry::Vacant(_) => unreachable!(),
        };

        OccupiedExtEntry {
            btree_map,
            entry,
        }
    }

    pub fn into_collection_mut(self) -> (&'a mut BTreeMap<K, V>, K) {
        let Self {
            mut btree_map,
            entry,
        } = self;

        let key = entry.into_key();

        // SAFETY: this is kept alive by the lifetime 'a,
        // and does not alias entry because it's dropped.
        (unsafe { btree_map.as_mut() }, key)
    }

    pub fn get_collection_ref(&self) -> &BTreeMap<K, V> {
        unsafe { self.btree_map.as_ref() }
    }
}

pub fn get_first_vacant<V>(map: &mut BTreeMap<usize, V>) -> VacantExtEntry<'_, usize, V> {
    let i = get_first_vacant_index(map);

    let vacant = match get_occupied(map, i) {
        Ok(_) => unreachable!(),
        Err(v) => v,
    };

    vacant
}

pub fn get_first_vacant_index<V>(map: &mut BTreeMap<usize, V>) -> usize {
    for (i, (k, _)) in map.iter().enumerate() {
        if i == *k {
            continue
        }
        return i
    }

    map.len()
}
