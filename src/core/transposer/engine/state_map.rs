use futures::{future::FusedFuture, Future};
use pin_project::pin_project;
use std::{
    marker::PhantomPinned,
    mem::MaybeUninit,
    pin::Pin,
    task::{Context, Poll},
};

use crate::core::Transposer;

use super::{
    engine_time::EngineTime, input_buffer::InputBuffer, lazy_state::LazyState,
    transposer_frame::TransposerFrame, transposer_update::TransposerUpdate,
};

// pointer needs to be the top argument as its target may have pointers into inputs or transposer.
#[pin_project]
pub struct UpdateItem<'a, T: Transposer> {
    #[pin]
    pub time: EngineTime<'a, T::Time>,
    // TODO: EngineTime and UpdateItemData both track the same thing. they probably should be merged.
    pub data: UpdateItemData<T>,
    pub events_emitted: EventsEmitted,
}

pub enum UpdateItemData<T: Transposer> {
    Init(Box<T>),
    Input(Box<[T::Input]>),
    Schedule,
}

pub enum EventsEmitted {
    Some,
    None,
    Pending,
}

impl EventsEmitted {
    pub fn any(&self) -> bool {
        match self {
            Self::Some => true,
            Self::None => false,
            Self::Pending => false,
        }
    }
    pub fn done(&self) -> bool {
        match self {
            Self::Some => true,
            Self::None => true,
            Self::Pending => false,
        }
    }
}

#[pin_project(project=BufferedItemProject)]
pub struct BufferedItem<'a, T: Transposer> {
    // this is first because it has references to other fields.
    // don't want this to dangle anything
    #[pin]
    pub update_future: MaybeUninit<TransposerUpdate<'a, T>>,

    // this has a mutable reference to it as long as update_future is not is_terminated
    pub transposer_frame: TransposerFrame<'a, T>,

    pub input_state: LazyState<T::InputState>,

    // update_future has references into both transposer_frame and input_state
    _marker: PhantomPinned,
}

impl<'a, T: Transposer> BufferedItem<'a, T> {
    pub fn new(transposer: T) -> BufferedItem<'a, T>
    where
        T: Clone,
    {
        BufferedItem {
            update_future: MaybeUninit::uninit(),
            transposer_frame: TransposerFrame::new(transposer),
            input_state: LazyState::new(),
            _marker: PhantomPinned,
        }
    }

    /// set up a new buffered item from the previous one.
    pub fn dup(&self) -> Self
    where
        T: Clone,
    {
        debug_assert!(self.is_terminated());

        BufferedItem {
            update_future: MaybeUninit::uninit(),
            transposer_frame: self.transposer_frame.clone(),
            input_state: LazyState::new(),
            _marker: PhantomPinned,
        }
    }

    /// modify an existing buffered item for reuse
    pub fn refurb(&mut self) {
        unsafe { self.update_future.assume_init_drop() };
        self.update_future = MaybeUninit::uninit();
        self.input_state = LazyState::new();
    }

    /// initialize the pointers and futures in a newly pinned buffered item.
    pub fn init(self: Pin<&mut Self>, update_item: &'a UpdateItem<'a, T>)
    where
        T: Clone,
    {
        let this = self.project();

        // SAFETY: we're going out and back into a pin to get instantiate maybeUninit, and then casting update_future to a regular mutable reference.
        let update_future = unsafe {
            let update_future: Pin<&mut MaybeUninit<TransposerUpdate<'a, T>>> = this.update_future;
            let update_future = Pin::into_inner_unchecked(update_future);
            *update_future = MaybeUninit::new(TransposerUpdate::new());
            let update_future = update_future.assume_init_mut();
            let update_future = Pin::new_unchecked(update_future);
            update_future
        };

        let transposer_frame: *mut _ = this.transposer_frame;
        // SAFETY: this is a self reference so it won't outlive its content.
        let transposer_frame: &mut _ = unsafe { transposer_frame.as_mut().unwrap() };

        let input_state: *mut _ = this.input_state;
        // SAFETY: this is a self reference so it won't outlive its content.
        let input_state: &mut _ = unsafe { input_state.as_mut().unwrap() };

        match (&update_item.time, &update_item.data) {
            (EngineTime::Init, UpdateItemData::Init(_)) => {
                debug_assert!(transposer_frame.init_next(update_item).is_none());
                update_future.init_init(transposer_frame, input_state);
            }
            (EngineTime::Input(time), UpdateItemData::Input(data)) => {
                debug_assert!(transposer_frame.init_next(update_item).is_none());
                update_future.init_input(transposer_frame, input_state, *time, data.as_ref());
            }
            (EngineTime::Schedule(time), UpdateItemData::Schedule) => {
                match transposer_frame.init_next(update_item) {
                    Some((time_again, event_payload)) => {
                        debug_assert!(*time == time_again);
                        update_future.init_schedule(
                            transposer_frame,
                            input_state,
                            time.time,
                            event_payload,
                        );
                    }
                    None => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn next_update_item(
        &self,
        input_buffer: &mut InputBuffer<T::Time, T::Input>,
    ) -> Option<UpdateItem<'a, T>>
    where
        T: Clone,
    {
        debug_assert!(self.is_terminated());

        let time = self.transposer_frame.get_next_time(input_buffer)?;

        let data = match time {
            EngineTime::Init => unreachable!(),
            EngineTime::Input(_) => {
                let (new_time, inputs) = input_buffer.pop_first().unwrap();
                debug_assert!(time.raw_time() == new_time);
                UpdateItemData::Input(inputs.into_boxed_slice())
            }
            EngineTime::Schedule(_) => UpdateItemData::Schedule,
        };

        Some(UpdateItem {
            time,
            data,
            events_emitted: EventsEmitted::Pending,
        })
    }
}

impl<'a, T: Transposer> Future for BufferedItem<'a, T> {
    type Output = Vec<T::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let BufferedItemProject {
            update_future,
            input_state,
            ..
        } = self.project();

        // SAFETY: we go right back into the pin. and we are assuming that init has been run.
        let update_future = unsafe { Pin::into_inner_unchecked(update_future) };
        let update_future = unsafe { update_future.assume_init_mut() };
        let update_future = unsafe { Pin::new_unchecked(update_future) };
        let update_future: Pin<&mut TransposerUpdate<'a, T>> = update_future;

        let input_state: &mut LazyState<T::InputState> = input_state;

        if update_future.is_terminated() {
            return Poll::Pending;
        }

        if input_state.requested() {
            return Poll::Pending;
        }

        match update_future.poll(cx) {
            Poll::Ready(result) => {
                // TODO handle exit result
                Poll::Ready(result.outputs)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<'a, T: Transposer> FusedFuture for BufferedItem<'a, T> {
    fn is_terminated(&self) -> bool {
        // SAFETY: init must be run before this, but it should have been.
        unsafe { self.update_future.assume_init_ref() }.is_terminated()
    }
}
