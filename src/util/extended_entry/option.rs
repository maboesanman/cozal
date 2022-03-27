use std::hint::unreachable_unchecked;
use std::ptr::NonNull;

pub fn get_occupied<T>(
    this: &mut Option<T>,
) -> Result<OccupiedExtEntry<'_, T>, VacantExtEntry<'_, T>> {
    let option: NonNull<Option<T>> = this.into();

    match this {
        Some(inner) => Ok(OccupiedExtEntry {
            option,
            inner,
        }),
        None => Err(VacantExtEntry {
            option: this
        }),
    }
}

#[derive(Debug)]
pub struct OccupiedExtEntry<'a, T> {
    option: NonNull<Option<T>>,
    inner:  &'a mut T,
}

impl<'a, T> OccupiedExtEntry<'a, T> {
    pub fn get_value(&self) -> &T {
        self.inner
    }

    pub fn get_value_mut(&mut self) -> &mut T {
        self.inner
    }

    pub fn into_value_mut(self) -> &'a mut T {
        self.inner
    }

    pub fn vacate(self) -> (VacantExtEntry<'a, T>, T) {
        todo!()
    }

    pub fn into_collection_mut(self) -> &'a mut Option<T> {
        let Self {
            inner,
            mut option,
        } = self;

        drop(inner);

        // SAFETY: this is kept alive by the lifetime 'a,
        // and does not alias entry because it's dropped.
        unsafe { option.as_mut() }
    }
}

#[derive(Debug)]
pub struct VacantExtEntry<'a, T> {
    option: &'a mut Option<T>,
}

impl<'a, T> VacantExtEntry<'a, T> {
    pub fn occupy(self, value: T) -> OccupiedExtEntry<'a, T> {
        let option: NonNull<Option<T>> = self.option.into();

        match self.option {
            Some(inner) => OccupiedExtEntry {
                option,
                inner,
            },
            None => unsafe { unreachable_unchecked() },
        }
    }

    pub fn into_collection_mut(self) -> &'a mut Option<T> {
        self.option
    }
}
