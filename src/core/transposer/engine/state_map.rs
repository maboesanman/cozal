use std::{cmp::{self, Ordering}, marker::PhantomPinned, mem::MaybeUninit, pin::Pin, task::{Context, Poll}};
use pin_project::pin_project;
use futures::{Future, future::{FusedFuture}};

use crate::core::Transposer;

use super::{engine_time::EngineTime, input_buffer::InputBuffer, lazy_state::LazyState, sparse_buffer_stack::SparseBufferStack, transposer_frame::TransposerFrame, transposer_update::TransposerUpdate};

// #[pin_project]
// pub struct StateMap<'map, T: Transposer + Clone + 'map, const N: usize>
// where T::Scheduled: Clone {
//     #[pin]
//     inner: SparseBufferStack<'map, UpdateItem<'map, T>, BufferedItem<'map, T>, N>
// }

// impl<'map, T: Transposer + Clone + 'map, const N: usize> StateMap<'map, T, N>
// where T::Scheduled: Clone {
//     pub fn rollback(
//         self: Pin<&mut Self>,
//         rollback_time: T::Time,
//     ) -> Option<T::Time> {
//         let this = self.project();
//         let mut inner: Pin<&mut SparseBufferStack<'map, UpdateItem<'map, T>, BufferedItem<'map, T>, N>>;
//         inner = this.inner;

//         while inner.as_mut().peek().time.raw_time() <= rollback_time {
//             if inner.as_mut().pop().is_none() {
//                 break
//             }
//         }

//         // todo: this could possibly be later, which could reduce repeated work.
//         Some(rollback_time)
//     }

//     pub fn poll(
//         mut self: Pin<&mut Self>,
//         poll_time: T::Time,
//         input_state: T::InputState,
//         input_buffer: &mut InputBuffer<T::Time, T::Input>,
//         cx: &mut Context,
//     ) -> StateMapPoll<'map, T> {
//         match self.as_mut().poll_events(poll_time, input_buffer, cx) {
//             StateMapEventPoll::Ready => {
//                 let output_state = self.interpolate(poll_time, input_state).unwrap();
//                 StateMapPoll::Ready(output_state)
//             },
//             StateMapEventPoll::Outputs(t, o) => StateMapPoll::Outputs(t, o),
//             StateMapEventPoll::Rollback(t) => StateMapPoll::Rollback(t),
//             StateMapEventPoll::NeedsState(t) => StateMapPoll::NeedsState(t),
//             StateMapEventPoll::Pending => StateMapPoll::Pending,
//         }
//     }

//     pub fn poll_events(
//         mut self: Pin<&mut Self>,
//         poll_time: T::Time,
//         input_buffer: &mut InputBuffer<T::Time, T::Input>,
//         cx: &mut Context,
//     ) -> StateMapEventPoll<'map, T> {
//         if let Some(t) = self.as_mut().rollback_prep(poll_time, input_buffer) {
//             return StateMapEventPoll::Rollback(t);
//         }
//         let this = self.project();
//         let mut inner: Pin<&mut SparseBufferStack<'map, UpdateItem<'map, T>, BufferedItem<'map, T>, N>>;
//         inner = this.inner;

//         loop {
//             // find the most recent buffered state (and index)
//             let last_buffered_update_index = inner.as_mut().last_buffered_index_by(poll_time, |x| x.time.raw_time());
//             let is_current = last_buffered_update_index == inner.as_mut().len() - 1;
//             let (mut update_item, buffered_item) = inner.as_mut().get_pinned_mut(last_buffered_update_index).unwrap();
//             let mut buffered_item = buffered_item.unwrap().project();
//             let update_future = buffered_item.update_future.as_mut();
//             let input_state: &mut LazyState<_> = buffered_item.input_state;

            
//             match input_state {
//                 LazyState::Requested => {

//                 }
//                 _ => {

//                 }
//             }

//             // if we're still working, poll the update until we're stuck or ready.
//             if !update_future.is_terminated() {
//                 match update_future.poll(cx) {
//                     Poll::Ready(update_result) => {
//                         // if there are any output events we need to emit them.
//                         // note that when reconstructing a previous state, events_emitted is still true
//                         // from the original call. it is never set to false.
//                         if update_item.events_emitted && !update_result.outputs.is_empty() {
//                             update_item.as_mut().get_mut().events_emitted = true;
//                             break StateMapEventPoll::Outputs(
//                                 update_item.time.clone(),
//                                 update_result.outputs
//                             )
//                         }
//                     }
//                     Poll::Pending => match buffered_item.input_state {
//                         LazyState::Requested => break StateMapEventPoll::NeedsState(update_item.time.raw_time()),
//                         LazyState::Ready(_) => break StateMapEventPoll::Pending,
//                         LazyState::Pending => break StateMapEventPoll::Pending,
//                     }
//                 }
//             }

//             let transposer_frame: &mut TransposerFrame<'map, T> = buffered_item.transposer_frame;

//             if is_current {
//                 let next_input_time = input_buffer.first_time();
//                 let next_schedule_time = transposer_frame.get_next_schedule_time();

//                 let (time, data): (EngineTime<'map, T::Time>, UpdateItemData<T>) = match (next_input_time, next_schedule_time) {
//                     (None, None) => break StateMapEventPoll::Ready,
//                     (Some(t), None) => (EngineTime::new_input(t), UpdateItemData::Input(input_buffer.pop_first().unwrap().1.into_boxed_slice())),
//                     (None, Some(t)) => (EngineTime::from(t.clone()), UpdateItemData::Schedule),
//                     (Some(i), Some(s)) => match i.cmp(&s.time) {
//                         Ordering::Less => (EngineTime::new_input(i), UpdateItemData::Input(input_buffer.pop_first().unwrap().1.into_boxed_slice())),
//                         Ordering::Equal => (EngineTime::from(s.clone()), UpdateItemData::Schedule),
//                         Ordering::Greater => (EngineTime::from(s.clone()), UpdateItemData::Schedule),
//                     }
//                 };

//                 inner.as_mut().push(|_| UpdateItem {
//                     time,
//                     data,
//                     events_emitted: false,
//                 });
//             }

//             let next_frame_time = inner.as_mut().get(last_buffered_update_index + 1).unwrap().0.time.raw_time();

//             if next_frame_time > poll_time {
//                 break StateMapEventPoll::Ready;
//             }

//             // (re)create the next buffer item.
//             inner.as_mut().buffer(
//                 last_buffered_update_index + 1,
//                 |last_buffered_ref: &BufferedItem<T>| {
//                     let update_future = TransposerUpdate::new();
//                     let transposer_frame = last_buffered_ref.transposer_frame.clone();
//                     let input_state = LazyState::Pending;

//                     BufferedItem {
//                         update_future,
//                         transposer_frame,
//                         input_state,
//                         _marker: PhantomPinned,
//                     }
//                 },
//                 |buffer_ref: Pin<&mut BufferedItem<'map, T>>, update_ref: &UpdateItem<'map, T>| {
//                     let buffer_ref = buffer_ref.project();
//                     let update_future: Pin<&mut TransposerUpdate<'map, T>> = buffer_ref.update_future;

//                     let transposer_frame: &mut TransposerFrame<T> = buffer_ref.transposer_frame;
//                     let transposer_frame: *mut TransposerFrame<'map, T> = transposer_frame;
//                     // SAFETY: this is a self reference, and won't outlive its target.
//                     let transposer_frame: &'map mut TransposerFrame<'map, T> = unsafe { transposer_frame.as_mut() }.unwrap();
                    
//                     let input_state: &mut LazyState<T::InputState> = buffer_ref.input_state;
//                     let input_state: *mut LazyState<T::InputState> = input_state;
//                     // SAFETY: this is a self reference, and won't outlive its target.
//                     let input_state: &'map mut LazyState<T::InputState> = unsafe { input_state.as_mut() }.unwrap();

//                     transposer_frame.advance_time(&update_ref.time);
//                     match (&update_ref.time, &update_ref.data) {
//                         (EngineTime::Input(time), UpdateItemData::Input(inputs)) => {
//                             update_future.start_input(
//                                 transposer_frame,
//                                 input_state,
//                                 inputs
//                             );
//                         }
//                         (EngineTime::Schedule(time_schedule), UpdateItemData::Schedule) => {
//                             update_future.start_schedule(
//                                 transposer_frame,
//                                 input_state
//                             );
//                         }
//                         _ => {
//                             // everything else is either init (which can't be here)
//                             // or the engine time and update item data don't match.
//                             unreachable!()
//                         }
//                     }
//                 },
//             ).unwrap();
//         }
//     }

//     pub fn next(
//         self: Pin<&mut Self>,
//         poll_time: T::Time,
//         input_buffer: &mut InputBuffer<T::Time, T::Input>,
//     ) -> Option<StateMapNext<'map, T::Time>>{
//         let this = self.project();
//         let inner: Pin<&mut SparseBufferStack<'map, UpdateItem<'map, T>, BufferedItem<'map, T>, N>>;
//         inner = this.inner;

//         let next_input_time = input_buffer.first_time();

//         let last_buffered_update_index = inner.as_ref().last_buffered_index_by(poll_time, |x| x.time.raw_time());
//         let last_update_index = inner.as_ref().last_index_by(poll_time, |x| x.time.raw_time());
        
//         let (last_buffered_item, last_buffered_update) = inner.get_pinned_mut(last_buffered_update_index).unwrap();
//         let last_buffered_update = last_buffered_update.unwrap();
//         let update_future = last_buffered_update.project().update_future;

//         todo!()
//     }


//     fn rollback_prep(
//         self: Pin<&mut Self>,
//         poll_time: T::Time,
//         input_buffer: &mut InputBuffer<T::Time, T::Input>,
//     ) -> Option<T::Time> {
//         let mut this = self.project();
//         let inner: &mut Pin<&mut SparseBufferStack<'map, UpdateItem<'map, T>, BufferedItem<'map, T>, N>>;
//         inner = &mut this.inner;

//         let next_input_time = input_buffer.first_time();

//         let inputs_handled = next_input_time.map_or(true, |next_input_time| poll_time < next_input_time);

//         // rollback internally if we need to
//         if inputs_handled {
//             None
//         } else {
//             let mut needs_rollback: Option<T::Time> = None;
//             loop {
//                 let current_time = inner.peek().time.raw_time();
//                 if next_input_time.unwrap() <= current_time {
//                     // todo this unwrap is sketchy
//                     let UpdateItem {
//                         time,
//                         data,
//                         events_emitted,
//                     } = inner.as_mut().pop().unwrap();

//                     if let UpdateItemData::Input(inputs) = data {
//                         input_buffer.extend_front(time.raw_time(), inputs);
//                     }

//                     if events_emitted {
//                         needs_rollback = match needs_rollback {
//                             Some(t) => Some(cmp::min(t, time.raw_time())),
//                             None => Some(time.raw_time())
//                         };
//                     }
//                 } else {
//                     break;
//                 }
//             }

//             needs_rollback
//         }
//     }

//     fn interpolate(&self, poll_time: T::Time, input_state: T::InputState) -> Option<T::OutputState> {
//         let i = self.inner.last_index_by(poll_time, |x| x.time.raw_time());

//         let (item, buffer) = self.inner.get(i)?;
//         let buffer = buffer?;

//         if !buffer.update_future.is_terminated() {
//             return None;
//         }

//         Some(buffer.transposer_frame.transposer.interpolate(item.time.raw_time(), poll_time, input_state))
//     }
// }

// pointer needs to be the top argument as its target may have pointers into inputs or transposer.
#[pin_project]
pub struct UpdateItem<'a, T: Transposer> {
    #[pin]
    pub time: EngineTime<'a, T::Time>,
    // TODO: EngineTime and UpdateItemData both track the same thing. they probably should be merged.
    pub data: UpdateItemData<T>,
    pub events_emitted: bool,
}

pub enum UpdateItemData<T: Transposer> {
    Init(Box<T>),
    Input(Box<[T::Input]>),
    Schedule
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
        where T: Clone
    {
        BufferedItem {
            update_future: MaybeUninit::uninit(),
            transposer_frame: TransposerFrame::new(transposer),
            input_state: LazyState::Pending,
            _marker: PhantomPinned,
        }
    }

    /// set up a new buffered item from the previous one.
    pub fn dup(&self) -> Self
        where T: Clone
    {
        debug_assert!(self.is_terminated());

        BufferedItem {
            update_future: MaybeUninit::uninit(),
            transposer_frame: self.transposer_frame.clone(),
            input_state: LazyState::Pending,
            _marker: PhantomPinned,
        }
    }

    /// initialize the pointers and futures in a newly pinned buffered item.
    pub fn init(
        mut self: Pin<&'a mut Self>,
        update_item: &'a UpdateItem<'a, T>,
        input_buffer: &mut InputBuffer<T::Time, T::Input>
    )
        where T: Clone
    {
        let this = self.project();
        let update_future = unsafe {
            let update_future: Pin<&mut MaybeUninit<TransposerUpdate<'a, T>>> = this.update_future;
            let update_future = Pin::into_inner_unchecked(update_future);
            *update_future = MaybeUninit::new(TransposerUpdate::new());
            let update_future = update_future.assume_init_mut();
            let update_future = Pin::new_unchecked(update_future);
            update_future
        };
        let transposer_frame: &'a mut TransposerFrame<'a, T> = this.transposer_frame;
        let input_state: &'a mut LazyState<T::InputState> = this.input_state;

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
                        update_future.init_schedule(transposer_frame, input_state, time.time, event_payload);
                    }
                    None => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn next_update_item(&self, input_buffer: &mut InputBuffer<T::Time, T::Input>) -> Option<UpdateItem<'a, T>>
        where T: Clone
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
            EngineTime::Schedule(_) => {
                UpdateItemData::Schedule
            }
        };

        Some(UpdateItem{
            time,
            data,
            events_emitted: false,
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
            return Poll::Pending
        }

        if let LazyState::Requested = input_state {
            return Poll::Pending
        }

        match update_future.poll(cx) {
            Poll::Ready(result) => {
                // TODO handle exit result
                Poll::Ready(result.outputs)
            }
            Poll::Pending => Poll::Pending
        }
    }
}

impl<'a, T: Transposer> FusedFuture for BufferedItem<'a, T> {
    fn is_terminated(&self) -> bool {
        // SAFETY: init must be run before this, but it should have been.
        unsafe {
            self.update_future.assume_init_ref()
        }.is_terminated()
    }
}


// pub enum StateMapPoll<'map, T: Transposer> {
//     Ready(T::OutputState),
//     Outputs(EngineTime<'map, T::Time>, Vec<T::Output>),
//     Rollback(T::Time),
//     NeedsState(T::Time),
//     Pending,
// }

// pub enum StateMapEventPoll<'map, T: Transposer> {
//     Ready,
//     Outputs(EngineTime<'map, T::Time>, Vec<T::Output>),
//     Rollback(T::Time),
//     NeedsState(T::Time),
//     Pending,
// }

// pub struct StateMapNext<'map, T: Ord + Copy + Default> {
//     time: EngineTime<'map, T>,
//     index: usize,
//     stateful: bool,
// }