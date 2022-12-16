use core::future::Future;
use core::pin::Pin;
use std::any::Any;
use std::marker::PhantomData;

use super::update::{Arg, UpdateContext, WrappedTransposer};
use crate::schedule_storage::StorageFamily;
use crate::{Transposer, TransposerInput, TransposerInputEventHandler};

pub struct InitArg;
pub struct InitArgPassed;

impl<T: Transposer, S: StorageFamily> Arg<T, S> for InitArg {
    type Passed<'a> = InitArgPassed;

    // this must be extracted before the context is contstructed to avoid mutable alias
    fn prep<'a, 't>(
        &'a self,
        _: &'t mut WrappedTransposer<T, S>
    ) -> InitArgPassed {
        InitArgPassed
    }

    fn run<'a, C: UpdateContext<T, S>>(
        passed: Self::Passed<'a>,
        transposer: &'a mut T,
        context: &'a mut C,
        _time: T::Time,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        Box::pin(transposer.init(context))
    }
}

pub struct InputArg<I: TransposerInput> {
    inputs: [I::InputEvent]
}

pub struct InputArgPassed<'a> {
    inputs: &'a dyn Any
}

impl<T: Transposer, I: TransposerInput<Base=T>, S: StorageFamily> Arg<T, S> for InputArg<I>
where T: TransposerInputEventHandler<I>
{
    type Passed<'a> = InputArgPassed<'a>;

    // this must be extracted before the context is contstructed to avoid mutable alias
    fn prep<'a, 't>(
        &'a self,
        _: &'t mut WrappedTransposer<T, S>
    ) -> Self::Passed<'a> {
        // InputArgPassed {
        //     inputs: &self.inputs
        // }
        todo!()
    }

    fn run<'a, C: UpdateContext<T, S>>(
        passed: Self::Passed<'a>,
        transposer: &'a mut T,
        context: &'a mut C,
        time: T::Time,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        todo!()
        // Box::pin(transposer.handle_input(time, passed.inputs, context))
    }
}

pub struct ScheduledArg;

pub struct ScheduledArgPassed<T: Transposer> {
    payload: T::Scheduled
}

impl<T: Transposer, S: StorageFamily> Arg<T, S> for ScheduledArg {
    type Passed<'a> = ScheduledArgPassed<T>;

    fn prep<'a, 't>(
        &'a self,
        wrapped_transposer: &'t mut WrappedTransposer<T, S>
    ) -> Self::Passed<'a> {
        let (_, payload) = wrapped_transposer.pop_schedule_event().unwrap();
        ScheduledArgPassed {
            payload
        }
    }

    fn run<'a, C: UpdateContext<T, S>>(
        passed: Self::Passed<'a>,
        transposer: &'a mut T,
        context: &'a mut C,
        time: T::Time,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        Box::pin(transposer.handle_scheduled(time, passed.payload, context))
    }
}