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
    fn get_fut<'a>(
        transposer: &'a mut T,
        context: &'a mut UpdateContext<T>,
        time: T::Time,
        value: Self::Passed,
        storage_slot: &'a mut MaybeUninit<Self::Stored>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>>;

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

    fn get_fut<'a>(
        transposer: &'a mut T,
        context: &'a mut UpdateContext<T>,
        time: T::Time,
        value: Self::Passed,
        storage_slot: &'a mut MaybeUninit<Self::Stored>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        *storage_slot = MaybeUninit::new(());

        transposer.init(context)
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
    type Passed = Box<[T::Input]>;

    type Stored = Box<[T::Input]>;

    #[warn(unsafe_op_in_unsafe_fn)]
    fn get_fut<'a>(
        transposer: &'a mut T,
        context: &'a mut UpdateContext<T>,
        time: T::Time,
        value: Self::Passed,
        storage_slot: &'a mut MaybeUninit<Self::Stored>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        *storage_slot = MaybeUninit::new(value);

        // SAFETY: we just assigned this
        let storage_slot = unsafe { storage_slot.assume_init_mut() };
        transposer.handle_input(time, storage_slot, context)
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

    #[warn(unsafe_op_in_unsafe_fn)]
    fn get_fut<'a>(
        transposer: &'a mut T,
        context: &'a mut UpdateContext<T>,
        time: T::Time,
        value: Self::Passed,
        storage_slot: &'a mut MaybeUninit<Self::Stored>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        *storage_slot = MaybeUninit::new(());

        transposer.handle_scheduled(time, value, context)
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
