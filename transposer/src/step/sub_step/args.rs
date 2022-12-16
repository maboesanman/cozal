use core::future::Future;
use core::pin::Pin;
use std::marker::PhantomData;

use super::update::{Arg, UpdateContext, WrappedTransposer};
use crate::schedule_storage::StorageFamily;
use crate::{Transposer, TransposerInput, TransposerInputEventHandler};

pub struct InitArg<T: Transposer, S: StorageFamily> {
    phantom: PhantomData<fn() -> (T, S)>,
}

impl<T: Transposer, S: StorageFamily> Arg<T, S> for InitArg<T, S> {
    type Stored = ();
    type Referenced = ();
    type Passed<'a> = ()
    where Self::Referenced: 'a;

    async fn run<'a, C: UpdateContext<T, S>>(
        transposer: &'a mut T,
        context: &'a mut C,
        _time: T::Time,
        _value: Self::Passed<'a>,
    ) {
        transposer.init(context).await
    }

    fn get_passed<'a>(
        _frame: &mut WrappedTransposer<T, S>,
        _in_arg: &'a mut Self::Stored,
    ) { }
}

pub struct InputArg<T: Transposer, I: TransposerInput<Base=T>, S: StorageFamily>
where T: TransposerInputEventHandler<I>
{
    phantom: PhantomData<fn() -> (T, I, S)>,
}

impl<T: Transposer, I: TransposerInput<Base=T>, S: StorageFamily> Arg<T, S> for InputArg<T, I, S>
where T: TransposerInputEventHandler<I>
{
    type Stored = Box<[I::InputEvent]>;
    type Referenced = I::InputEvent;
    type Passed<'a> = &'a mut [I::InputEvent]
    where Self::Referenced: 'a;

    async fn run<'a, C: UpdateContext<T, S>>(
        transposer: &'a mut T,
        context: &'a mut C,
        time: T::Time,
        arg: Self::Passed<'a>,
    ) {
        transposer.handle_input(time, arg, context).await
    }

    fn get_passed<'a>(
        _frame: &mut WrappedTransposer<T, S>,
        in_arg: &'a mut Self::Stored,
    ) -> Self::Passed<'a>
    {
        in_arg
    }
}

pub struct ScheduledArg<T: Transposer, S: StorageFamily> {
    phantom: PhantomData<fn() -> (T, S)>,
}

impl<T: Transposer, S: StorageFamily> Arg<T, S> for ScheduledArg<T, S> {
    type Stored = ();
    type Referenced = ();
    type Passed<'a> = T::Scheduled
    where Self::Referenced: 'a;

    async fn run<'a, C: UpdateContext<T, S>>(
        transposer: &'a mut T,
        context: &'a mut C,
        time: T::Time,
        arg: Self::Passed<'a>,
    ) {
        transposer.handle_scheduled(time, arg, context).await
    }

    fn get_passed<'a>(
        frame: &mut WrappedTransposer<T, S>,
        _in_arg: &'a mut Self::Stored,
    ) -> Self::Passed<'a>
    {
        if let Some((_, payload)) = frame.pop_schedule_event() {
            payload
        } else {
            unreachable!()
        }
    }
}
