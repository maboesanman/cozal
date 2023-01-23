use core::cmp::Ordering;
use core::pin::Pin;
use core::task::Context;

use util::replace_mut;

use super::WrappedTransposer;
use super::args::{InitArg, InputArg, ScheduledArg};
use super::sub_step_update_context::{SubStepUpdateContext, SubStepUpdateContextFamily};
use super::time::SubStepTime;
use super::update::{Arg, Update, UpdateContext};
use crate::Transposer;
use crate::schedule_storage::{StorageFamily, RefCounted};
use crate::step::step::InputState;
use crate::step::step_inputs::StepInputs;

pub struct SubStep<'almost_static, T: Transposer, S: StorageFamily, Is: InputState<T>>
where (T, Is): 'almost_static
{
    time:        SubStepTime<T::Time>,
    inner:       SubStepInner<'almost_static, T, S, Is>,
    input_state: S::LazyState<Is>,

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

impl<'almost_static, T: Transposer, S: StorageFamily, Is: InputState<T>> SubStep<'almost_static, T, S, Is> {
    pub fn new_init(
        transposer: T,
        rng_seed: [u8; 32],
        input_state: S::LazyState<Is>,
    ) -> Self {
        let time = SubStepTime::new_init();
        let wrapped_transposer = WrappedTransposer::new(transposer, rng_seed);
        let wrapped_transposer = Box::new(wrapped_transposer);
        let wrapped_transposer = S::Transposer::<WrappedTransposer<T, S>>::new(wrapped_transposer);
        let update = Update::new::<SubStepUpdateContextFamily<T, S>>(
            wrapped_transposer,
            InitArg::new(),
            time.clone(),
            input_state.clone()
        );
        let inner = SubStepInner::SaturatingInit {
            update,
        };
        SubStep {
            time,
            inner,
            input_state,

            #[cfg(debug_assertions)]
            uuid_self: uuid::Uuid::new_v4(),
            #[cfg(debug_assertions)]
            uuid_prev: None,
        }
    }

    pub fn next_unsaturated(
        &self,
        next_inputs: &mut Option<StepInputs<T>>,
        input_state: S::LazyState<Is>,
    ) -> Result<Option<Self>, NextUnsaturatedErr> {
        #[cfg(debug_assertions)]
        if let Some(inputs) = next_inputs {
            let self_time = self.time().raw_time();
            if inputs.time() < self_time {
                return Err(NextUnsaturatedErr::InputPastOrPresent)
            }
            if inputs.time() == self_time && self.time().index() != 0 {
                return Err(NextUnsaturatedErr::InputPastOrPresent)
            }
        }

        let wrapped_transposer = match &self.inner {
            SubStepInner::SaturatedInit {
                wrapped_transposer,
                ..
            } => wrapped_transposer,
            SubStepInner::SaturatedInput {
                wrapped_transposer,
                ..
            } => wrapped_transposer,
            SubStepInner::SaturatedScheduled {
                wrapped_transposer,
                ..
            } => wrapped_transposer,
            _ => return Err(NextUnsaturatedErr::NotSaturated),
        };
        let next_scheduled_time = wrapped_transposer.get_next_scheduled_time();

        let next_time_index = self.time.index() + 1;

        let (time, inner) = match (next_inputs.as_ref().map(|x| x.time()), next_scheduled_time) {
            (None, None) => return Ok(None),
            (None, Some(next_scheduled_time)) => (
                SubStepTime::new_scheduled(next_time_index, next_scheduled_time.clone()),
                SubStepInner::OriginalUnsaturatedScheduled
            ),
            (Some(t_i), None) => (
                SubStepTime::new_input(next_time_index, t_i),
                SubStepInner::OriginalUnsaturatedInput {
                    inputs: core::mem::take(next_inputs).unwrap(),
                }
            ),
            (Some(t_i), Some(t_s)) => match t_i.cmp(&t_s.time) {
                Ordering::Greater => (
                    SubStepTime::new_scheduled(next_time_index, t_s.clone()),
                    SubStepInner::OriginalUnsaturatedScheduled,
                ),
                _ => {
                    (SubStepTime::new_input(next_time_index, t_i), SubStepInner::OriginalUnsaturatedInput {
                    inputs: core::mem::take(next_inputs).unwrap(),
                })
                },
            },
        };

        let item = SubStep {
            time,
            inner,
            input_state,

            #[cfg(debug_assertions)]
            uuid_self: uuid::Uuid::new_v4(),
            #[cfg(debug_assertions)]
            uuid_prev: Some(self.uuid_self),
        };

        Ok(Some(item))
    }

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

    pub fn time(&self) -> &SubStepTime<T::Time> {
        &self.time
    }

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

// arg types
type InitUpdate<'almost_static, T, S> = Update<'almost_static, T, S, SubStepUpdateContext<'almost_static, T, S>, InitArg<T>>;
type InputUpdate<'almost_static, T, S> = Update<'almost_static, T, S, SubStepUpdateContext<'almost_static, T, S>, InputArg<T>>;
type ScheduledUpdate<'almost_static, T, S> = Update<'almost_static, T, S, SubStepUpdateContext<'almost_static, T, S>, ScheduledArg<T>>;

enum SubStepInner<'almost_static, T: Transposer, S: StorageFamily, Is: InputState<T>>
where (T, Is): 'almost_static
{
    // notably this can never be rehydrated because you need the preceding wrapped_transposer
    // and there isn't one, because this is init.
    UnsaturatedInit,
    OriginalUnsaturatedInput {
        inputs: StepInputs<T>,
    },
    OriginalUnsaturatedScheduled,
    RepeatUnsaturatedInput {
        inputs: StepInputs<T>,
    },
    RepeatUnsaturatedScheduled,
    SaturatingInit {
        update: Update<'almost_static, T, S, InitArg<T>, Is>,
    },
    OriginalSaturatingInput {
        update: Update<'almost_static, T, S, InputArg<T>, Is>,
    },
    OriginalSaturatingScheduled {
        update: Update<'almost_static, T, S, ScheduledArg<T>, Is>,
    },
    RepeatSaturatingInput {
        update: Update<'almost_static, T, S, InputArg<T>, Is>,
    },
    RepeatSaturatingScheduled {
        update: Update<'almost_static, T, S, ScheduledArg<T>, Is>,
    },
    SaturatedInit {
        wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    },
    SaturatedInput {
        inputs: StepInputs<T>,
        wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    },
    SaturatedScheduled {
        wrapped_transposer: S::Transposer<WrappedTransposer<T, S>>,
    }
}

impl<'almost_static, T: Transposer, S: StorageFamily, Is: InputState<T>> SubStepInner<'almost_static, T, S, Is> {
    fn recover() -> Self {
        Self::UnsaturatedInit
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
