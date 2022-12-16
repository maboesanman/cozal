use core::future::Future;
use core::pin::Pin;

use super::{UpdateContext, WrappedTransposer};
use crate::schedule_storage::StorageFamily;
use crate::Transposer;

pub trait Arg<T: Transposer, S: StorageFamily> {
    type Stored: Unpin;
    type Referenced;
    type Passed<'a>
    where Self::Referenced: 'a;

    fn get_passed<'a>(
        frame: &mut WrappedTransposer<T, S>,
        borrowed: &'a mut Self::Stored,
    ) -> Self::Passed<'a>;

    async fn run<'a, C: UpdateContext<T, S>>(
        transposer: &'a mut T,
        context: &'a mut C,
        time: T::Time,
        arg: Self::Passed<'a>,
    );
}
