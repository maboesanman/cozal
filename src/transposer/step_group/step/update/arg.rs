use core::mem::MaybeUninit;
use core::pin::Pin;

use futures_core::Future;

use super::{UpdateContext, WrappedTransposer};
use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::Transposer;

pub trait Arg<T: Transposer, S: StorageFamily> {
    type Passed;
    type Stored;

    // STORAGE MUST BE VALID AFTER THIS
    fn get_fut<'a, C: UpdateContext<T, S>>(
        transposer: &'a mut T,
        context: &'a mut C,
        time: T::Time,
        value: Self::Passed,
        storage_slot: &'a mut MaybeUninit<Self::Stored>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>>;

    fn get_arg(frame: &mut WrappedTransposer<T, S>, in_arg: Self::Stored) -> Self::Passed;

    fn get_stored(passed: Self::Passed) -> Self::Stored;
}
