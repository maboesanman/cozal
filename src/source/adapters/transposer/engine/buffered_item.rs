use core::future::Future;
use futures_core::FusedFuture;
use pin_project::pin_project;
use core::{
    marker::PhantomPinned,
    pin::Pin,
    task::{Context, Poll},
};

use super::super::Transposer;

use super::{
    engine_time::EngineTime, input_buffer::InputBuffer, lazy_state::LazyState,
    transposer_frame::TransposerFrame, transposer_update::TransposerUpdate,
    update_item::UpdateItem, update_item::UpdateItemData,
};

#[pin_project(project=BufferedItemProject)]
pub struct BufferedItem<'a, T: Transposer> {
    // this is first because it has references to other fields.
    // don't want this to dangle anything
    #[pin]
    state: BufferedItemState<'a, T>,

    // this has a mutable reference to it as long as update_future is not is_terminated
    pub transposer_frame: TransposerFrame<'a, T>,

    pub input_state: LazyState<T::InputState>,

    // update_future has references into both transposer_frame and input_state
    // transposer_frame is already !Unpin, but just in case that changes, this is !Unpin for other reasons.
    _marker: PhantomPinned,
}

#[pin_project(project=BufferedItemStateProject)]
enum BufferedItemState<'a, T: Transposer> {
    Unpollable {
        update_item: &'a UpdateItem<'a, T>,
    },
    Pollable {
        #[pin]
        update_future: TransposerUpdate<'a, T>,
    },
}

impl<'a, T: Transposer> BufferedItem<'a, T> {
    #[allow(unused)]
    pub fn new(
        transposer: T,
        update_item: &'a UpdateItem<'a, T>,
        rng_seed: [u8; 32],
    ) -> BufferedItem<'a, T>
    where
        T: Clone,
    {
        BufferedItem {
            state: BufferedItemState::Unpollable { update_item },
            transposer_frame: TransposerFrame::new(transposer, rng_seed),
            input_state: LazyState::new(),
            _marker: PhantomPinned,
        }
    }

    /// set up a new buffered item from the previous one.
    #[allow(unused)]
    pub fn dup(&self, update_item: &'a UpdateItem<'a, T>) -> Self
    where
        T: Clone,
    {
        debug_assert!(self.is_terminated());

        BufferedItem {
            state: BufferedItemState::Unpollable { update_item },
            transposer_frame: self.transposer_frame.clone(),
            input_state: LazyState::new(),
            _marker: PhantomPinned,
        }
    }

    /// modify an existing buffered item for reuse
    #[allow(unused)]
    pub fn refurb(&mut self, update_item: &'a UpdateItem<'a, T>) {
        debug_assert!(self.is_terminated());

        self.state = BufferedItemState::Unpollable { update_item };
        self.input_state = LazyState::new();
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

        Some(UpdateItem::new(time, data))
    }
}

impl<'a, T: Transposer> Future for BufferedItem<'a, T> {
    type Output = Vec<T::Output>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        loop {
            let BufferedItemProject {
                mut state,
                input_state,
                transposer_frame,
                ..
            } = self.as_mut().project();
            match state.as_mut().project() {
                BufferedItemStateProject::Unpollable { update_item } => {
                    let update_item: &'a UpdateItem<'a, T> = update_item;

                    let transposer_frame: *mut _ = transposer_frame;
                    // SAFETY: this is a self reference so it won't outlive its content.
                    let transposer_frame: &mut _ = unsafe { transposer_frame.as_mut().unwrap() };

                    let input_state: *mut _ = input_state;
                    // SAFETY: this is a self reference so it won't outlive its content.
                    let input_state: &mut _ = unsafe { input_state.as_mut().unwrap() };

                    let update_future = match (&update_item.time, &update_item.data) {
                        (EngineTime::Init, UpdateItemData::Init(_)) => {
                            debug_assert!(transposer_frame.init_next(update_item).is_none());
                            TransposerUpdate::new_init(transposer_frame, input_state)
                        }
                        (EngineTime::Input(time), UpdateItemData::Input(data)) => {
                            debug_assert!(transposer_frame.init_next(update_item).is_none());
                            TransposerUpdate::new_input(
                                transposer_frame,
                                input_state,
                                *time,
                                data.as_ref(),
                            )
                        }
                        (EngineTime::Schedule(time), UpdateItemData::Schedule) => {
                            match transposer_frame.init_next(update_item) {
                                Some((time_again, event_payload)) => {
                                    debug_assert!(*time == time_again);
                                    TransposerUpdate::new_scheduled(
                                        transposer_frame,
                                        input_state,
                                        time.time,
                                        event_payload,
                                    )
                                }
                                None => unreachable!(),
                            }
                        }
                        _ => unreachable!(),
                    };
                    // SAFETY: we're changing between enum variants, and no !Unpin fields are moved.
                    let state = unsafe { state.get_unchecked_mut() };
                    *state = BufferedItemState::Pollable { update_future };
                }
                BufferedItemStateProject::Pollable { update_future } => {
                    let update_future: Pin<&mut TransposerUpdate<'a, T>> = update_future;
                    break match update_future.poll(cx) {
                        Poll::Ready(result) => Poll::Ready(result.outputs),
                        Poll::Pending => Poll::Pending,
                    };
                }
            };
        }
    }
}

impl<'a, T: Transposer> FusedFuture for BufferedItem<'a, T> {
    fn is_terminated(&self) -> bool {
        if let BufferedItemState::Pollable { update_future } = &self.state {
            update_future.is_terminated()
        } else {
            false
        }
    }
}
