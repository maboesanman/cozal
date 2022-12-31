use core::future::Future;
use core::pin::Pin;

use super::{UpdateContext, WrappedTransposer};
use crate::schedule_storage::StorageFamily;
use crate::Transposer;

pub trait Arg<T: Transposer, S: StorageFamily, C: UpdateContext<T, S>> {
    // this must be extracted before the context is contstructed to avoid mutable alias
    fn prep<'a, 't>(
        &'a self,
        time: T::Time,
        wrapped_transposer: &'t mut WrappedTransposer<T, S>
    ) -> Box<dyn FnOnce(&'a mut T, &'a mut C) -> Pin<Box<dyn Future<Output = ()> + 'a>> + 'a>;
}

impl<T: Transposer, S: StorageFamily, C: UpdateContext<T, S>, A: Arg<T, S, C> + ?Sized> Arg<T, S, C> for Box<A> {
    fn prep<'a, 't>(
        &'a self,
        time: <T as Transposer>::Time,
        wrapped_transposer: &'t mut WrappedTransposer<T, S>
    ) -> Box<dyn FnOnce(&'a mut T, &'a mut C) -> Pin<Box<dyn Future<Output = ()> + 'a>> + 'a> {
        A::prep(&self, time, wrapped_transposer)
    }
}

pub fn convert<
    T: Transposer,
    S: StorageFamily,
    C1: UpdateContext<T, S>,
    C2: UpdateContext<T, S>,
    A1: Arg<T, S, C1>,
    A2: Arg<T, S, C2>,
>(arg: A1) -> A2 {
    todo!()
}