use std::collections::VecDeque;
use std::task::Poll;

use transposer::step::{NoInput, NoInputManager, Step, StepPoll};
use transposer::Transposer;

use self::channels::{interpolation_future, CallerChannelStatus, ChannelStatuses};
use self::steps::Steps;
use crate::source_poll::{self, TrySourcePoll};
use crate::sources::no_input_transposer::channels::original_step_future::OriginalStepPoll;
use crate::traits::SourceContext;
use crate::{Source, SourcePoll};

mod channels;
mod steps;

pub struct NoInputTransposerSource<T: Transposer<InputStateManager = NoInputManager>> {
    steps: Steps<T>,

    channel_statuses: ChannelStatuses<T>,
}

impl<T: Transposer<InputStateManager = NoInputManager>> NoInputTransposerSource<T> {}

impl<T: Transposer<InputStateManager = NoInputManager>> Source for NoInputTransposerSource<T> {
    type Time = T::Time;

    type Event = T::OutputEvent;

    type State = T::OutputState;

    type Error = ();

    fn poll(
        &mut self,
        time: Self::Time,
        cx: SourceContext,
    ) -> TrySourcePoll<Self::Time, Self::Event, Self::State, Self::Error> {
        let SourceContext {
            channel: caller_channel,
            one_channel_waker,
            all_channel_waker,
        } = cx;

        let mut current_state = self.channel_statuses.get_channel_status(caller_channel);

        loop {
            current_state = match current_state {
                CallerChannelStatus::Free(free) => {
                    let pinned_times = free.get_pinned_times();

                    match self.steps.get_before_or_at(time, &pinned_times).unwrap() {
                        steps::BeforeStatus::Saturated {
                            step,
                            next_time,
                        } => {
                            let interpolation = step.interpolate(time).unwrap();
                            let interpolation = free.start_interpolation(interpolation, time);
                            CallerChannelStatus::InterpolationFuture(interpolation)
                        },
                        steps::BeforeStatus::Saturating {
                            step,
                            step_index,
                        } => {
                            if step.can_produce_events() {
                                let original_entry = free.start_original_step(time);
                                CallerChannelStatus::OriginalStepFuture(original_entry)
                            } else {
                                let repeat_entry = free.start_repeat_step(step_index, time);
                                CallerChannelStatus::RepeatStepFuture(repeat_entry)
                            }
                        },
                    }
                },
                CallerChannelStatus::InterpolationFuture(interpolation) => {
                    let prev_time = interpolation.caller_channel.get_value().poll_time;
                    if prev_time != time {
                        current_state = CallerChannelStatus::Free(interpolation.abandon());
                        continue
                    }

                    let (new_state, poll) = interpolation.poll(&one_channel_waker);
                    return Ok(match poll {
                        Poll::Pending => SourcePoll::Pending,
                        Poll::Ready(state) => SourcePoll::Ready {
                            state,
                            next_event_at: self.steps.get_scheduled_time(),
                        },
                    })
                },
                CallerChannelStatus::OriginalStepFuture(original) => {
                    let prev_time = original.caller_channel.get_value().poll_time;
                    if prev_time != time {
                        current_state = CallerChannelStatus::Free(original.abandon());
                        continue
                    }

                    let step = self.steps.get_last_mut();

                    let free = match original.poll(step, &all_channel_waker) {
                        OriginalStepPoll::OutputEvent(event) => {
                            return Ok(SourcePoll::Interrupt {
                                time:      step.get_time(),
                                interrupt: crate::source_poll::Interrupt::Event(event),
                            })
                        },
                        OriginalStepPoll::Pending => return Ok(SourcePoll::Pending),
                        OriginalStepPoll::Free(free) => free,
                    };

                    CallerChannelStatus::Free(free)
                },
                CallerChannelStatus::RepeatStepFuture(repeat) => {
                    let block = repeat.caller_channel.get_value();
                    let step_id = block.unwrap_repeat_step();
                    let prev_time = block.poll_time;
                    if prev_time != time {
                        current_state = CallerChannelStatus::Free(repeat.abandon());
                        continue
                    }

                    let step = self.steps.get_mut_by_sequence_number(step_id).unwrap();

                    let free = match repeat.poll(step, &one_channel_waker) {
                        Poll::Pending => return Ok(SourcePoll::Pending),
                        Poll::Ready(free) => free,
                    };

                    CallerChannelStatus::Free(free)
                },
                CallerChannelStatus::Limbo => panic!(),
            };
        }
    }

    fn poll_events(
        &mut self,
        time: Self::Time,
        all_channel_waker: std::task::Waker,
    ) -> TrySourcePoll<Self::Time, Self::Event, (), Self::Error> {
        let pinned_times = self.channel_statuses.get_pinned_times();

        let poll = loop {
            let (poll, time) = match self
                .steps
                .get_before_or_at_events(time, &pinned_times)
                .unwrap()
            {
                steps::BeforeStatusEvents::Ready {
                    next_time,
                } => {
                    break SourcePoll::Ready {
                        state:         (),
                        next_event_at: next_time,
                    }
                },
                steps::BeforeStatusEvents::Saturating {
                    step, ..
                } => (step.poll(&all_channel_waker).unwrap(), step.get_time()),
            };

            match poll {
                StepPoll::Emitted(e) => {
                    break SourcePoll::Interrupt {
                        time,
                        interrupt: source_poll::Interrupt::Event(e),
                    }
                },
                StepPoll::Pending => break SourcePoll::Pending,
                StepPoll::Ready => continue,
            }
        };

        Ok(poll)
    }

    fn release_channel(&mut self, channel: usize) {
        let current_state = self.channel_statuses.get_channel_status(channel);

        let _: channels::free::Free<T> = match current_state {
            CallerChannelStatus::Free(f) => f,
            CallerChannelStatus::InterpolationFuture(i) => i.abandon(),
            CallerChannelStatus::OriginalStepFuture(o) => o.abandon(),
            CallerChannelStatus::RepeatStepFuture(r) => r.abandon(),
            CallerChannelStatus::Limbo => panic!(),
        };
    }

    fn advance(&mut self, time: Self::Time) {
        self.steps.delete_before(time)
    }

    fn max_channel(&self) -> std::num::NonZeroUsize {
        std::num::NonZeroUsize::MAX
    }
}
