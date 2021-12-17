use core::pin::Pin;

use futures_core::Future;

use super::{UpdateContext, WrappedTransposer};
use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::Transposer;

pub trait Arg<T: Transposer, S: StorageFamily> {
    type Stored: Unpin;
    type Passed<'a>
    where
        T::Input: 'a;

    fn get_passed<'a>(
        frame: &mut WrappedTransposer<T, S>,
        borrowed: &'a mut Self::Stored,
    ) -> Self::Passed<'a>
    where
        T::Input: 'a;

    fn get_future<'a, C: UpdateContext<T, S>>(
        transposer: &'a mut T,
        context: &'a mut C,
        time: T::Time,
        arg: Self::Passed<'a>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>>;
}
