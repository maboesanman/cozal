use super::{UpdateContext, WrappedTransposer};
use crate::schedule_storage::StorageFamily;
use crate::Transposer;

pub trait Arg<T: Transposer, S: StorageFamily>: Unpin {
    type Passed<'a>
    where
        T: 'a;

    fn get_passed<'a>(&'a self, frame: &mut WrappedTransposer<T, S>) -> Self::Passed<'a>
    where
        T: 'a;

    async fn run<'a, C: UpdateContext<'a, T, S>>(
        transposer: &'a mut T,
        context: C,
        time: T::Time,
        passed: Self::Passed<'a>,
    ) where
        T: 'a;
}
