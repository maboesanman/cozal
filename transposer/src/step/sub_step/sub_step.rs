use core::cmp::Ordering;
use core::pin::Pin;
use core::task::Context;

use util::replace_mut;

use super::WrappedTransposer;
use super::args::{InitArg, InputArg, ScheduledArg};
use super::sub_step_update_context::{AsyncCollector, DiscardCollector, SubStepUpdateContext};
use super::time::SubStepTime;
use super::update::{Arg, Update, UpdateContext};
use crate::Transposer;
use crate::schedule_storage::StorageFamily;

pub struct SubStep<T: Transposer, S: StorageFamily> {
    time:        SubStepTime<T::Time>,
    inner:       SubStepInner<T, S>,
    // input_state: S::LazyState<>,

    // these are used purely for enforcing that saturate calls use the previous step.
    #[cfg(debug_assertions)]
    uuid_self: uuid::Uuid,
    #[cfg(debug_assertions)]
    uuid_prev: Option<uuid::Uuid>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum NextUnsaturatedErr {
    NotSaturated,
    #[cfg(debug_assertions)]
    InputPastOrPresent,
}
#[derive(Debug, PartialEq, Eq)]
pub enum SaturateErr {
    PreviousNotSaturated,
    SelfNotUnsaturated,
    #[cfg(debug_assertions)]
    IncorrectPrevious,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DesaturateErr {
    AlreadyUnsaturated,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PollErr {
    Unsaturated,
    Saturated,
}

impl<T: Transposer, S: StorageFamily> SubStep<T, S> {
    // pub fn new_init(
    //     transposer: T,
    //     rng_seed: [u8; 32],
    //     input_state: &LazyState<T::InputState>,
    // ) -> Self {
    //     let time = SubStepTime::new_init();
    //     let wrapped_transposer = WrappedTransposer::new(transposer, rng_seed);
    //     let wrapped_transposer =
    //         <S::Transposer<WrappedTransposer<T, S>> as TransposerPointer<_>>::new(
    //             wrapped_transposer,
    //         );
    //     let input_state = S::LazyState::new(input_state.get_proxy());
    //     let update = Update::new(wrapped_transposer, (), time.clone(), input_state.clone());
    //     let inner = SubStepInner::SaturatingInit {
    //         update,
    //     };
    //     SubStep {
    //         time,
    //         inner,
    //         input_state,

    //         #[cfg(debug_assertions)]
    //         uuid_self: uuid::Uuid::new_v4(),
    //         #[cfg(debug_assertions)]
    //         uuid_prev: None,
    //     }
    // }

//     pub fn next_unsaturated(
//         &self,
//         next_inputs: &mut NextInputs<T>,
//         input_state: &LazyState<T::InputState>,
//     ) -> Result<Option<Self>, NextUnsaturatedErr> {
//         #[cfg(debug_assertions)]
//         if let Some((t, _)) = next_inputs {
//             let self_time = self.time().raw_time();
//             if *t < self_time {
//                 return Err(NextUnsaturatedErr::InputPastOrPresent)
//             }
//             if *t == self_time && self.time().index() != 0 {
//                 return Err(NextUnsaturatedErr::InputPastOrPresent)
//             }
//         }

//         let wrapped_transposer = match &self.inner {
//             SubStepInner::SaturatedInit {
//                 wrapped_transposer,
//                 ..
//             } => wrapped_transposer,
//             SubStepInner::SaturatedInput {
//                 wrapped_transposer,
//                 ..
//             } => wrapped_transposer,
//             SubStepInner::SaturatedScheduled {
//                 wrapped_transposer,
//                 ..
//             } => wrapped_transposer,
//             _ => return Err(NextUnsaturatedErr::NotSaturated),
//         };
//         let next_scheduled_time = wrapped_transposer.get_next_scheduled_time();

//         let next_time_index = self.time.index() + 1;

//         let (time, inputs) = match (&next_inputs, next_scheduled_time) {
//             (None, None) => return Ok(None),
//             (None, Some(t)) => (SubStepTime::new_scheduled(next_time_index, t.clone()), None),
//             (Some((t, _)), None) => {
//                 let time = *t;
//                 let inputs = core::mem::take(next_inputs).map(|(_, i)| i);
//                 (SubStepTime::new_input(next_time_index, time), inputs)
//             },
//             (Some((t_i, _)), Some(t_s)) => match t_i.cmp(&t_s.time) {
//                 Ordering::Greater => (
//                     SubStepTime::new_scheduled(next_time_index, t_s.clone()),
//                     None,
//                 ),
//                 _ => {
//                     let time = *t_i;
//                     let inputs = core::mem::take(next_inputs).map(|(_, i)| i);
//                     (SubStepTime::new_input(next_time_index, time), inputs)
//                 },
//             },
//         };

//         let inner = if let Some(inputs) = inputs {
//             SubStepInner::OriginalUnsaturatedInput {
//                 inputs,
//             }
//         } else {
//             SubStepInner::OriginalUnsaturatedScheduled
//         };

//         let input_state = S::LazyState::new(input_state.get_proxy());

//         let item = SubStep {
//             time,
//             inner,
//             input_state,

//             #[cfg(debug_assertions)]
//             uuid_self: uuid::Uuid::new_v4(),
//             #[cfg(debug_assertions)]
//             uuid_prev: Some(self.uuid_self),
//         };

//         Ok(Some(item))
//     }

//     pub fn next_unsaturated_same_time(&self) -> Result<Option<Self>, NextUnsaturatedErr> {
//         // init is always its own time.
//         if self.time().index() == 0 {
//             return Ok(None)
//         }

//         let wrapped_transposer = match &self.inner {
//             SubStepInner::SaturatedInit {
//                 wrapped_transposer,
//                 ..
//             } => wrapped_transposer,
//             SubStepInner::SaturatedInput {
//                 wrapped_transposer,
//                 ..
//             } => wrapped_transposer,
//             SubStepInner::SaturatedScheduled {
//                 wrapped_transposer,
//                 ..
//             } => wrapped_transposer,
//             _ => return Err(NextUnsaturatedErr::NotSaturated),
//         };

//         let next_scheduled_time = wrapped_transposer.get_next_scheduled_time();
//         let next_scheduled_time = match next_scheduled_time {
//             Some(t) => t,
//             None => return Ok(None),
//         };

//         if self.time().raw_time() != next_scheduled_time.time {
//             return Ok(None)
//         }

//         let next_time_index = self.time.index() + 1;
//         let time = SubStepTime::new_scheduled(next_time_index, next_scheduled_time.clone());
//         let inner = SubStepInner::OriginalUnsaturatedScheduled;

//         let item = SubStep {
//             time,
//             inner,
//             input_state: self.input_state.clone(),

//             #[cfg(debug_assertions)]
//             uuid_self: uuid::Uuid::new_v4(),
//             #[cfg(debug_assertions)]
//             uuid_prev: Some(self.uuid_self),
//         };

//         Ok(Some(item))
//     }

//     // previous is expected to be the value produced this via next_unsaturated.
//     pub fn saturate_take(&mut self, previous: &mut Self) -> Result<(), SaturateErr> {
//         #[cfg(debug_assertions)]
//         if self.uuid_prev != Some(previous.uuid_self) {
//             return Err(SaturateErr::IncorrectPrevious)
//         }

//         if !previous.inner.is_saturated() {
//             return Err(SaturateErr::PreviousNotSaturated)
//         }

//         if !self.inner.is_unsaturated() {
//             return Err(SaturateErr::SelfNotUnsaturated)
//         }

//         let wrapped_transposer = replace_mut::replace_and_return(
//             &mut previous.inner,
//             || SubStepInner::Unreachable,
//             |prev| match prev {
//                 SubStepInner::SaturatedInit {
//                     wrapped_transposer,
//                 } => (SubStepInner::UnsaturatedInit, wrapped_transposer),
//                 SubStepInner::SaturatedInput {
//                     inputs,
//                     wrapped_transposer,
//                 } => (
//                     SubStepInner::RepeatUnsaturatedInput {
//                         inputs,
//                     },
//                     wrapped_transposer,
//                 ),
//                 SubStepInner::SaturatedScheduled {
//                     wrapped_transposer,
//                 } => (SubStepInner::RepeatUnsaturatedScheduled, wrapped_transposer),
//                 _ => unreachable!(),
//             },
//         );

//         self.saturate_from_wrapped_transposer(wrapped_transposer);

//         Ok(())
//     }

//     // previous is expected to be the value produced this via next_unsaturated.
//     pub fn saturate_clone(&mut self, previous: &Self) -> Result<(), SaturateErr>
//     where
//         S::Transposer<WrappedTransposer<T, S>>: Clone,
//     {
//         #[cfg(debug_assertions)]
//         if self.uuid_prev != Some(previous.uuid_self) {
//             return Err(SaturateErr::IncorrectPrevious)
//         }

//         if !self.inner.is_unsaturated() {
//             return Err(SaturateErr::SelfNotUnsaturated)
//         }

//         let wrapped_transposer: S::Transposer<WrappedTransposer<T, S>> = match &previous.inner {
//             SubStepInner::SaturatedInit {
//                 wrapped_transposer,
//                 ..
//             } => Ok(wrapped_transposer.clone()),
//             SubStepInner::SaturatedInput {
//                 wrapped_transposer,
//                 ..
//             } => Ok(wrapped_transposer.clone()),
//             SubStepInner::SaturatedScheduled {
//                 wrapped_transposer,
//             } => Ok(wrapped_transposer.clone()),
//             _ => Err(SaturateErr::PreviousNotSaturated),
//         }?;

//         self.saturate_from_wrapped_transposer(wrapped_transposer);

//         Ok(())
//     }

//     // panics if self is unsaturated.
//     fn saturate_from_wrapped_transposer(
//         &mut self,
//         wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
//     ) {
//         replace_mut::replace(
//             &mut self.inner,
//             || SubStepInner::Unreachable,
//             |next| match next {
//                 SubStepInner::OriginalUnsaturatedInput {
//                     inputs,
//                 } => {
//                     let update = Update::new(
//                         wrapped_transposer,
//                         inputs,
//                         self.time.clone(),
//                         self.input_state.clone(),
//                     );
//                     SubStepInner::OriginalSaturatingInput {
//                         update,
//                     }
//                 },
//                 SubStepInner::RepeatUnsaturatedInput {
//                     inputs,
//                 } => {
//                     let update = Update::new(
//                         wrapped_transposer,
//                         inputs,
//                         self.time.clone(),
//                         self.input_state.clone(),
//                     );
//                     SubStepInner::RepeatSaturatingInput {
//                         update,
//                     }
//                 },
//                 SubStepInner::OriginalUnsaturatedScheduled => {
//                     let update = Update::new(
//                         wrapped_transposer,
//                         (),
//                         self.time.clone(),
//                         self.input_state.clone(),
//                     );
//                     SubStepInner::OriginalSaturatingScheduled {
//                         update,
//                     }
//                 },
//                 SubStepInner::RepeatUnsaturatedScheduled => {
//                     let update = Update::new(
//                         wrapped_transposer,
//                         (),
//                         self.time.clone(),
//                         self.input_state.clone(),
//                     );
//                     SubStepInner::RepeatSaturatingScheduled {
//                         update,
//                     }
//                 },
//                 _ => unreachable!(),
//             },
//         );
//     }

//     pub fn desaturate(&mut self) -> Result<Option<T>, DesaturateErr> {
//         replace_mut::replace_and_return(&mut self.inner, SubStepInner::recover, |original| {
//             match original {
//                 SubStepInner::OriginalSaturatingInput {
//                     update,
//                 } => {
//                     let inner = SubStepInner::OriginalUnsaturatedInput {
//                         inputs: update.reclaim_pending(),
//                     };
//                     (inner, Ok(None))
//                 },
//                 SubStepInner::RepeatSaturatingInput {
//                     update,
//                 } => {
//                     let inner = SubStepInner::RepeatUnsaturatedInput {
//                         inputs: update.reclaim_pending(),
//                     };
//                     (inner, Ok(None))
//                 },
//                 SubStepInner::OriginalSaturatingScheduled {
//                     update: _,
//                 } => (SubStepInner::OriginalUnsaturatedScheduled, Ok(None)),
//                 SubStepInner::RepeatSaturatingScheduled {
//                     update: _,
//                 } => (SubStepInner::RepeatUnsaturatedScheduled, Ok(None)),
//                 SubStepInner::SaturatedInit {
//                     wrapped_transposer,
//                 } => (
//                     SubStepInner::UnsaturatedInit,
//                     Ok(wrapped_transposer.try_take().map(|w_t| w_t.transposer)),
//                 ),
//                 SubStepInner::SaturatedInput {
//                     inputs,
//                     wrapped_transposer,
//                 } => (
//                     SubStepInner::RepeatUnsaturatedInput {
//                         inputs,
//                     },
//                     Ok(wrapped_transposer.try_take().map(|w_t| w_t.transposer)),
//                 ),
//                 SubStepInner::SaturatedScheduled {
//                     wrapped_transposer,
//                 } => (
//                     SubStepInner::RepeatUnsaturatedScheduled,
//                     Ok(wrapped_transposer.try_take().map(|w_t| w_t.transposer)),
//                 ),
//                 other => (other, Err(DesaturateErr::AlreadyUnsaturated)),
//             }
//         })
//     }

//     pub fn time(&self) -> &SubStepTime<T::Time> {
//         &self.time
//     }

//     // this has the additional gurantee that the pointer returned lives until this is desaturated or dropped.
//     // if you move self the returned pointer is still valid.
//     pub fn finished_wrapped_transposer(&self) -> Option<&S::Transposer<WrappedTransposer<T, S>>> {
//         match &self.inner {
//             SubStepInner::SaturatedInit {
//                 wrapped_transposer,
//             } => Some(wrapped_transposer),
//             SubStepInner::SaturatedInput {
//                 wrapped_transposer,
//                 ..
//             } => Some(wrapped_transposer),
//             SubStepInner::SaturatedScheduled {
//                 wrapped_transposer,
//             } => Some(wrapped_transposer),
//             _ => None,
//         }
//     }

//     pub fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Result<StepPoll<T>, PollErr> {
//         replace_mut::replace_and_return(
//             &mut self.inner,
//             || SubStepInner::Unreachable,
//             |inner| match inner {
//                 SubStepInner::SaturatingInit {
//                     mut update,
//                 } => match Pin::new(&mut update).poll(cx) {
//                     UpdatePoll::Pending => (
//                         SubStepInner::SaturatingInit {
//                             update,
//                         },
//                         Ok(StepPoll::Pending),
//                     ),
//                     UpdatePoll::Event(output) => (
//                         SubStepInner::SaturatingInit {
//                             update,
//                         },
//                         Ok(StepPoll::Emitted(output)),
//                     ),
//                     UpdatePoll::Ready(wrapped_transposer) => (
//                         SubStepInner::SaturatedInit {
//                             wrapped_transposer,
//                         },
//                         Ok(StepPoll::Ready),
//                     ),
//                 },
//                 SubStepInner::OriginalSaturatingInput {
//                     mut update,
//                 } => match Pin::new(&mut update).poll(cx) {
//                     UpdatePoll::Pending => (
//                         SubStepInner::OriginalSaturatingInput {
//                             update,
//                         },
//                         Ok(StepPoll::Pending),
//                     ),
//                     UpdatePoll::Event(output) => (
//                         SubStepInner::OriginalSaturatingInput {
//                             update,
//                         },
//                         Ok(StepPoll::Emitted(output)),
//                     ),
//                     UpdatePoll::Ready(wrapped_transposer) => {
//                         let inputs = update.reclaim_pending();
//                         (
//                             SubStepInner::SaturatedInput {
//                                 wrapped_transposer,
//                                 inputs,
//                             },
//                             Ok(StepPoll::Ready),
//                         )
//                     },
//                 },
//                 SubStepInner::RepeatSaturatingInput {
//                     mut update,
//                 } => match Pin::new(&mut update).poll(cx) {
//                     UpdatePoll::Pending => (
//                         SubStepInner::RepeatSaturatingInput {
//                             update,
//                         },
//                         Ok(StepPoll::Pending),
//                     ),
//                     UpdatePoll::Event(_) => unreachable!(),
//                     UpdatePoll::Ready(wrapped_transposer) => {
//                         let inputs = update.reclaim_pending();
//                         (
//                             SubStepInner::SaturatedInput {
//                                 wrapped_transposer,
//                                 inputs,
//                             },
//                             Ok(StepPoll::Ready),
//                         )
//                     },
//                 },
//                 SubStepInner::OriginalSaturatingScheduled {
//                     mut update,
//                 } => match Pin::new(&mut update).poll(cx) {
//                     UpdatePoll::Pending => (
//                         SubStepInner::OriginalSaturatingScheduled {
//                             update,
//                         },
//                         Ok(StepPoll::Pending),
//                     ),
//                     UpdatePoll::Event(output) => (
//                         SubStepInner::OriginalSaturatingScheduled {
//                             update,
//                         },
//                         Ok(StepPoll::Emitted(output)),
//                     ),
//                     UpdatePoll::Ready(wrapped_transposer) => (
//                         SubStepInner::SaturatedScheduled {
//                             wrapped_transposer,
//                         },
//                         Ok(StepPoll::Ready),
//                     ),
//                 },
//                 SubStepInner::RepeatSaturatingScheduled {
//                     mut update,
//                 } => match Pin::new(&mut update).poll(cx) {
//                     UpdatePoll::Pending => (
//                         SubStepInner::RepeatSaturatingScheduled {
//                             update,
//                         },
//                         Ok(StepPoll::Pending),
//                     ),
//                     UpdatePoll::Event(_) => unreachable!(),
//                     UpdatePoll::Ready(wrapped_transposer) => (
//                         SubStepInner::SaturatedScheduled {
//                             wrapped_transposer,
//                         },
//                         Ok(StepPoll::Ready),
//                     ),
//                 },
//                 SubStepInner::UnsaturatedInit
//                 | SubStepInner::OriginalUnsaturatedInput {
//                     ..
//                 }
//                 | SubStepInner::OriginalUnsaturatedScheduled
//                 | SubStepInner::RepeatUnsaturatedInput {
//                     ..
//                 }
//                 | SubStepInner::RepeatUnsaturatedScheduled => (inner, Err(PollErr::Unsaturated)),
//                 SubStepInner::SaturatedInit {
//                     ..
//                 }
//                 | SubStepInner::SaturatedInput {
//                     ..
//                 }
//                 | SubStepInner::SaturatedScheduled {
//                     ..
//                 } => (inner, Err(PollErr::Saturated)),
//                 SubStepInner::Unreachable => unreachable!(),
//             },
//         )
//     }
}

// context types
type OriginalContext<T, S> = SubStepUpdateContext<T, S, AsyncCollector<<T as Transposer>::OutputEvent>>;
type RepeatContext<T, S> = SubStepUpdateContext<T, S, DiscardCollector>;

// arg types
type InitUpdate<T, S> = Update<T, S, OriginalContext<T, S>, InitArg>;
type InputUpdate<T, S, C> = Update<T, S, C, Box<dyn Arg<T, S, C>>>;
type ScheduledUpdate<T, S, C> = Update<T, S, C, ScheduledArg>;

// arg + context types
type OriginalInputUpdate<T, S> = InputUpdate<T, S, OriginalContext<T, S>>;
type OriginalScheduledUpdate<T, S> = ScheduledUpdate<T, S, OriginalContext<T, S>>;
type RepeatInputUpdate<T, S> = InputUpdate<T, S, RepeatContext<T, S>>;
type RepeatScheduledUpdate<T, S> = ScheduledUpdate<T, S, RepeatContext<T, S>>;

enum SubStepInner<T: Transposer, S: StorageFamily> {
    // notably this can never be rehydrated because you need the preceding wrapped_transposer
    // and there isn't one, because this is init.
    UnsaturatedInit,
    OriginalUnsaturatedInput {
        inputs: Box<dyn Arg<T, S, OriginalContext<T, S>>>,
    },
    OriginalUnsaturatedScheduled,
    RepeatUnsaturatedInput {
        inputs: Box<dyn Arg<T, S, RepeatContext<T, S>>>,
    },
    RepeatUnsaturatedScheduled,
    SaturatingInit {
        update: InitUpdate<T, S>,
    },
    OriginalSaturatingInput {
        update: OriginalInputUpdate<T, S>,
    },
    OriginalSaturatingScheduled {
        update: OriginalScheduledUpdate<T, S>,
    },
    RepeatSaturatingInput {
        update: RepeatInputUpdate<T, S>,
    },
    RepeatSaturatingScheduled {
        update: RepeatScheduledUpdate<T, S>,
    },
    SaturatedInit {
        wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    },
    SaturatedInput {
        inputs: Box<dyn Arg<T, S, RepeatContext<T, S>>>,
        wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    },
    SaturatedScheduled {
        wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    },
    Unreachable,
}

impl<T: Transposer, S: StorageFamily> SubStepInner<T, S> {
    fn recover() -> Self {
        Self::Unreachable
    }

    pub fn is_unsaturated(&self) -> bool {
        matches!(
            self,
            SubStepInner::OriginalUnsaturatedInput { .. }
                | SubStepInner::OriginalUnsaturatedScheduled
                | SubStepInner::RepeatUnsaturatedInput { .. }
                | SubStepInner::RepeatUnsaturatedScheduled
        )
    }

    pub fn is_saturated(&self) -> bool {
        matches!(
            self,
            SubStepInner::SaturatedInit { .. }
                | SubStepInner::SaturatedInput { .. }
                | SubStepInner::SaturatedScheduled { .. }
        )
    }
}
