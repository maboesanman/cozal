use core::future::Future;
use core::pin::Pin;

use super::{UpdateContext, WrappedTransposer};
use crate::schedule_storage::StorageFamily;
use crate::Transposer;

pub trait Arg<T: Transposer, S: StorageFamily> {
    type Passed<'a>
    where Self: 'a;

    // this must be extracted before the context is contstructed to avoid mutable alias
    fn prep<'a, 't>(
        &'a self,
        wrapped_transposer: &'t mut WrappedTransposer<T, S>
    ) -> Self::Passed<'a>;

    // we then use the passed value
    fn run<'a, C: UpdateContext<T, S>>(
        passed: Self::Passed<'a>,
        transposer: &'a mut T,
        context: &'a mut C,
        time: T::Time,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>>;
}
