use core::future::Future;
use core::pin::Pin;
use std::marker::PhantomData;

use super::update::{Arg, UpdateContext, WrappedTransposer};
use crate::schedule_storage::StorageFamily;
use crate::step::step_inputs::StepInputs;
use crate::{Transposer, TransposerInput, TransposerInputEventHandler};

pub struct InitArg<T: Transposer>(PhantomData<StepInputs<T>>);

impl<T: Transposer> InitArg<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: Transposer, S: StorageFamily> Arg<T, S> for InitArg<T> {
    type Passed<'a> = ()
    where T: 'a;

    fn get_passed<'a>(&'a self, _frame: &mut WrappedTransposer<T, S>) -> Self::Passed<'a>
    where
        T: 'a,
    {
    }

    async fn run<'a, C: UpdateContext<'a, T, S>>(
        transposer: &'a mut T,
        mut context: C,
        _time: T::Time,
        _passed: Self::Passed<'a>,
    ) where
        T: 'a,
    {
        transposer.init(&mut context).await;
    }
}

pub struct InputArg<T: Transposer>(StepInputs<T>);

impl<T: Transposer> InputArg<T> {
    pub fn new(inputs: StepInputs<T>) -> Self {
        Self(inputs)
    }

    pub fn into_inputs(self) -> StepInputs<T> {
        self.0
    }
}

impl<T: Transposer, S: StorageFamily> Arg<T, S> for InputArg<T> {
    type Passed<'a> = &'a StepInputs<T>
    where T: 'a;

    fn get_passed<'a>(&'a self, _frame: &mut WrappedTransposer<T, S>) -> Self::Passed<'a>
    where
        T: 'a,
    {
        &self.0
    }

    async fn run<'a, C: UpdateContext<'a, T, S>>(
        transposer: &'a mut T,
        mut context: C,
        _time: <T as Transposer>::Time,
        passed: Self::Passed<'a>,
    ) where
        T: 'a,
    {
        passed.handle(transposer, &mut context).await
    }
}

pub struct ScheduledArg<T: Transposer>(PhantomData<StepInputs<T>>);

impl<T: Transposer> ScheduledArg<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: Transposer, S: StorageFamily> Arg<T, S> for ScheduledArg<T> {
    type Passed<'a> = T::Scheduled
    where T: 'a, StepInputs<T>: 'a;

    fn get_passed<'a>(&'a self, frame: &mut WrappedTransposer<T, S>) -> Self::Passed<'a>
    where
        T: 'a,
    {
        let (_, payload) = frame.pop_schedule_event().unwrap();

        payload
    }

    async fn run<'a, C: UpdateContext<'a, T, S>>(
        transposer: &'a mut T,
        mut context: C,
        time: <T as Transposer>::Time,
        passed: Self::Passed<'a>,
    ) where
        T: 'a,
    {
        transposer
            .handle_scheduled(time, passed, &mut context)
            .await;
    }
}
