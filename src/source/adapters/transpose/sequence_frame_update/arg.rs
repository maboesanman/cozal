use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::pin::Pin;

use futures_core::Future;

use super::frame::Frame;
use super::update_context::UpdateContext;
use crate::transposer::Transposer;

pub trait Arg<T: Transposer> {
    type Passed;
    type Stored;

    // STORAGE MUST BE VALID AFTER THIS
    fn get_fut<'a, C: UpdateContext<T>>(
        transposer: &'a mut T,
        context: &'a mut C,
        time: T::Time,
        value: Self::Passed,
        storage_slot: &'a mut MaybeUninit<Self::Stored>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>>;

    fn get_arg(frame: &mut Frame<T>, in_arg: Self::Stored) -> Self::Passed;

    fn get_stored(passed: Self::Passed) -> Self::Stored;
}

pub struct InitArg<T: Transposer>(PhantomData<T>);
impl<T: Transposer> Arg<T> for InitArg<T> {
    type Passed = ();

    type Stored = ();

    fn get_fut<'a, C: UpdateContext<T>>(
        transposer: &'a mut T,
        context: &'a mut C,
        _time: T::Time,
        _value: Self::Passed,
        storage_slot: &'a mut MaybeUninit<Self::Stored>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        *storage_slot = MaybeUninit::new(());

        transposer.init(context)
    }

    fn get_arg(_frame: &mut Frame<T>, _in_arg: Self::Stored) -> Self::Passed {}

    fn get_stored(_: Self::Passed) -> Self::Stored {}
}

pub struct InputArg<T: Transposer>(PhantomData<T>);
impl<T: Transposer> Arg<T> for InputArg<T> {
    type Passed = Box<[T::Input]>;

    type Stored = Box<[T::Input]>;

    fn get_fut<'a, C: UpdateContext<T>>(
        transposer: &'a mut T,
        context: &'a mut C,
        time: T::Time,
        value: Self::Passed,
        storage_slot: &'a mut MaybeUninit<Self::Stored>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        *storage_slot = MaybeUninit::new(value);

        // SAFETY: we just assigned this
        let storage_slot = unsafe { storage_slot.assume_init_mut() };
        transposer.handle_input(time, storage_slot, context)
    }

    fn get_arg(_frame: &mut Frame<T>, in_arg: Self::Stored) -> Self::Passed {
        in_arg
    }
    fn get_stored(passed: Self::Passed) -> Self::Stored {
        passed
    }
}

pub struct ScheduledArg<T: Transposer>(PhantomData<T>);
impl<T: Transposer> Arg<T> for ScheduledArg<T> {
    type Passed = T::Scheduled;

    type Stored = ();

    fn get_fut<'a, C: UpdateContext<T>>(
        transposer: &'a mut T,
        context: &'a mut C,
        time: T::Time,
        value: Self::Passed,
        storage_slot: &'a mut MaybeUninit<Self::Stored>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        *storage_slot = MaybeUninit::new(());

        transposer.handle_scheduled(time, value, context)
    }

    fn get_arg(frame: &mut Frame<T>, _in_arg: Self::Stored) -> Self::Passed {
        let val = frame.pop_schedule_event();

        debug_assert!(val.is_some());

        let (_, payload) = val.unwrap();

        payload
    }

    fn get_stored(_: Self::Passed) -> Self::Stored {}
}
