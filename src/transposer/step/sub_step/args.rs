use core::pin::Pin;
use std::marker::PhantomData;

use futures_core::Future;

use super::update::{Arg, UpdateContext, WrappedTransposer};
use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::Transposer;
use crate::util::debug_assert::debug_unreachable;

pub struct InitArg<T: Transposer, S: StorageFamily> {
    phantom: PhantomData<fn() -> (T, S)>,
}

impl<T: Transposer, S: StorageFamily> Arg<T, S> for InitArg<T, S> {
    type Stored = ();
    type Passed<'a>
    where
        T::Input: 'a,
    = ();

    fn get_future<'a, C: UpdateContext<T, S>>(
        transposer: &'a mut T,
        context: &'a mut C,
        _time: T::Time,
        _value: Self::Passed<'a>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        transposer.init(context)
    }

    fn get_passed<'a>(
        _frame: &mut WrappedTransposer<T, S>,
        _in_arg: &'a mut Self::Stored,
    ) -> Self::Passed<'a>
    where
        T::Input: 'a,
    {
    }
}

pub struct InputArg<T: Transposer, S: StorageFamily> {
    phantom: PhantomData<fn() -> (T, S)>,
}

impl<T: Transposer, S: StorageFamily> Arg<T, S> for InputArg<T, S> {
    type Stored = Box<[T::Input]>;
    type Passed<'a>
    where
        T::Input: 'a,
    = &'a mut [T::Input];

    fn get_future<'a, C: UpdateContext<T, S>>(
        transposer: &'a mut T,
        context: &'a mut C,
        time: T::Time,
        arg: Self::Passed<'a>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        transposer.handle_input(time, arg, context)
    }

    fn get_passed<'a>(
        _frame: &mut WrappedTransposer<T, S>,
        in_arg: &'a mut Self::Stored,
    ) -> Self::Passed<'a>
    where
        T::Input: 'a,
    {
        in_arg
    }
}

pub struct ScheduledArg<T: Transposer, S: StorageFamily> {
    phantom: PhantomData<fn() -> (T, S)>,
}

impl<T: Transposer, S: StorageFamily> Arg<T, S> for ScheduledArg<T, S> {
    type Stored = ();
    type Passed<'a>
    where
        T::Input: 'a,
    = T::Scheduled;

    fn get_future<'a, C: UpdateContext<T, S>>(
        transposer: &'a mut T,
        context: &'a mut C,
        time: T::Time,
        arg: Self::Passed<'a>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        transposer.handle_scheduled(time, arg, context)
    }

    fn get_passed<'a>(
        frame: &mut WrappedTransposer<T, S>,
        _in_arg: &'a mut Self::Stored,
    ) -> Self::Passed<'a>
    where
        T::Input: 'a,
    {
        if let Some((_, payload)) = frame.pop_schedule_event() {
            payload
        } else {
            unreachable!()
        }
    }
}
