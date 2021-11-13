use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::pin::Pin;

use futures_core::Future;

use super::update_context::UpdateContext;
use crate::source::adapters::transpose::engine_time::EngineTime;
use crate::source::adapters::transpose::frame::Frame;
use crate::transposer::Transposer;

pub trait Arg<T: Transposer> {
    type Passed;
    type Stored;

    // STORAGE MUST BE VALID AFTER THIS
    unsafe fn get_fut(
        transposer: &mut T,
        context: &mut UpdateContext<T>,
        time: T::Time,
        value: Self::Passed,
        storage_slot: &mut MaybeUninit<Self::Stored>,
    ) -> Pin<Box<dyn Future<Output = ()>>>;

    fn get_arg(
        frame: &mut Frame<T>,
        in_arg: Self::Stored,
        time: &EngineTime<T::Time>,
    ) -> Self::Passed;

    fn get_stored(passed: Self::Passed) -> Self::Stored;
}

pub struct InitArg<T: Transposer>(PhantomData<T>);
impl<T: Transposer> Arg<T> for InitArg<T> {
    type Passed = ();

    type Stored = ();

    unsafe fn get_fut(
        transposer: &mut T,
        context: &mut UpdateContext<T>,
        _time: T::Time,
        _value: Self::Passed,
        storage_slot: &mut MaybeUninit<Self::Stored>,
    ) -> Pin<Box<dyn Future<Output = ()>>> {
        *storage_slot = MaybeUninit::new(());

        let fut = transposer.init(context);
        core::mem::transmute(fut)
    }

    fn get_arg(
        _frame: &mut Frame<T>,
        _in_arg: Self::Stored,
        time: &EngineTime<T::Time>,
    ) -> Self::Passed {
        debug_assert!(time.is_init());
    }

    fn get_stored(_: Self::Passed) -> Self::Stored {
        ()
    }
}

pub struct InputArg<T: Transposer>(PhantomData<T>);
impl<T: Transposer> Arg<T> for InputArg<T> {
    type Passed = Vec<T::Input>;

    type Stored = Vec<T::Input>;

    unsafe fn get_fut(
        transposer: &mut T,
        context: &mut UpdateContext<T>,
        time: T::Time,
        value: Self::Passed,
        storage_slot: &mut MaybeUninit<Self::Stored>,
    ) -> Pin<Box<dyn Future<Output = ()>>> {
        *storage_slot = MaybeUninit::new(value);

        let inputs_ptr: *const _ = storage_slot.assume_init_ref();
        let inputs_ref = inputs_ptr.as_ref().unwrap();

        let fut = transposer.handle_input(time, inputs_ref, context);
        core::mem::transmute(fut)
    }

    fn get_arg(
        frame: &mut Frame<T>,
        in_arg: Self::Stored,
        time: &EngineTime<T::Time>,
    ) -> Self::Passed {
        debug_assert!(time.is_input());

        debug_assert!(if let Some(nst) = frame.get_next_scheduled_time() {
            match time.raw_time() {
                Ok(time) => {
                    if time < nst.time {
                        true
                    } else if time == nst.time {
                        true
                    } else {
                        false
                    }
                },
                Err(_) => false,
            }
        } else {
            true
        });

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

    unsafe fn get_fut(
        transposer: &mut T,
        context: &mut UpdateContext<T>,
        time: T::Time,
        value: Self::Passed,
        storage_slot: &mut MaybeUninit<Self::Stored>,
    ) -> Pin<Box<dyn Future<Output = ()>>> {
        *storage_slot = MaybeUninit::new(());

        let fut = transposer.handle_scheduled(time, value, context);
        core::mem::transmute(fut)
    }

    fn get_arg(
        frame: &mut Frame<T>,
        _in_arg: Self::Stored,
        time: &EngineTime<T::Time>,
    ) -> Self::Passed {
        debug_assert!(time.is_scheduled());

        let val = frame.pop_schedule_event();

        debug_assert!(val.is_some());

        let (t, payload) = val.unwrap();

        debug_assert!(time.equals_scheduled(&t));

        payload
    }

    fn get_stored(_: Self::Passed) -> Self::Stored {
        ()
    }
}
