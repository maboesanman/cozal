use core::future::Future;
use core::pin::Pin;

use super::update::{Arg, UpdateContext, WrappedTransposer};
use crate::schedule_storage::StorageFamily;
use crate::{Transposer, TransposerInput, TransposerInputEventHandler};

pub struct InitArg;

impl<T: Transposer, S: StorageFamily, C: UpdateContext<T, S>> Arg<T, S, C> for InitArg {
    // this must be extracted before the context is contstructed to avoid mutable alias
    fn prep<'a, 't>(
        &'a self,
        _time: T::Time,
        _wrapped_transposer: &'t mut WrappedTransposer<T, S>
    ) -> Box<dyn FnOnce(&'a mut T, &'a mut C) -> Pin<Box<dyn Future<Output = ()> + 'a>> + 'a> {
        Box::new(|transposer, context| Box::pin(transposer.init(context)))
    }
}

pub struct InputArg<I: TransposerInput> {
    inputs: [I::InputEvent]
}

impl<I: TransposerInput, S: StorageFamily, C: UpdateContext<I::Base, S>> Arg<I::Base, S, C> for InputArg<I> {
    fn prep<'a, 't>(
        &'a self,
        time: <I::Base as Transposer>::Time,
        _wrapped_transposer: &'t mut WrappedTransposer<I::Base, S>
    ) -> Box<dyn FnOnce(&'a mut I::Base, &'a mut C) -> Pin<Box<dyn Future<Output = ()> + 'a>> + 'a> {
        Box::new(move |transposer, context| Box::pin(transposer.handle_input(time, &self.inputs, context)))
    }
}

pub struct ScheduledArg;

impl<T: Transposer, S: StorageFamily, C: UpdateContext<T, S>> Arg<T, S, C> for ScheduledArg {
    fn prep<'a, 't>(
        &'a self,
        time: T::Time,
        wrapped_transposer: &'t mut WrappedTransposer<T, S>
    ) -> Box<dyn FnOnce(&'a mut T, &'a mut C) -> Pin<Box<dyn Future<Output = ()> + 'a>> + 'a> {
        let (_, payload) = wrapped_transposer.pop_schedule_event().unwrap();
        
        Box::new(move |transposer, context| Box::pin(transposer.handle_scheduled(time, payload, context)))
    }
}