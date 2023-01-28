use super::sub_step_update_context::SubStepUpdateContext;
use super::transposer_metadata::TransposerMetaData;
use crate::schedule_storage::{RefCounted, StorageFamily};
use crate::step::step_inputs::StepInputs;
use crate::step::InputState;
use crate::Transposer;

#[derive(Clone)]
pub struct WrappedTransposer<T: Transposer, S: StorageFamily> {
    pub transposer: T,
    pub metadata:   TransposerMetaData<T, S>,
}

impl<T: Transposer, S: StorageFamily> WrappedTransposer<T, S> {
    /// create a wrapped transposer, and perform all T::default scheduled events.
    pub async fn init<Is: InputState<T>>(
        mut transposer: T,
        rng_seed: [u8; 32],
        input_state: S::LazyState<Is>,
        outputs_to_swallow: usize,
        output_sender: futures_channel::mpsc::Sender<(
            T::OutputEvent,
            futures_channel::oneshot::Sender<()>,
        )>,
    ) -> S::Transposer<Self> {
        let mut metadata = TransposerMetaData::new(rng_seed);
        let input_state_provider = input_state.get_provider();
        let mut context = SubStepUpdateContext::new(
            &mut metadata,
            input_state_provider,
            outputs_to_swallow,
            output_sender,
        );

        transposer.init(&mut context).await;

        let SubStepUpdateContext {
            outputs_to_swallow,
            output_sender,
            ..
        } = context;

        let mut new = Self {
            transposer,
            metadata,
        };

        new.handle_scheduled(
            T::Time::default(),
            input_state,
            outputs_to_swallow,
            output_sender,
        )
        .await;

        S::Transposer::new(Box::new(new))
    }

    /// handle an input, and all scheduled events that occur at the same time.
    pub async fn handle_input<Is: InputState<T>>(
        &mut self,
        input: &StepInputs<T>,
        input_state: S::LazyState<Is>,
        outputs_to_swallow: usize,
        output_sender: futures_channel::mpsc::Sender<(
            T::OutputEvent,
            futures_channel::oneshot::Sender<()>,
        )>,
    ) {
        let input_state_provider = input_state.get_provider();
        let mut context = SubStepUpdateContext::new(
            &mut self.metadata,
            input_state_provider,
            outputs_to_swallow,
            output_sender,
        );

        context.metadata.last_updated.time = input.time();
        context.metadata.last_updated.index += 1;
        input.handle(&mut self.transposer, &mut context).await;

        let SubStepUpdateContext {
            output_sender,
            outputs_to_swallow,
            ..
        } = context;

        self.handle_scheduled(input.time(), input_state, outputs_to_swallow, output_sender)
            .await;
    }

    /// handle all scheduled events occuring at `time` (if any)
    pub async fn handle_scheduled<Is: InputState<T>>(
        &mut self,
        time: T::Time,
        input_state: S::LazyState<Is>,
        outputs_to_swallow: usize,
        output_sender: futures_channel::mpsc::Sender<(
            T::OutputEvent,
            futures_channel::oneshot::Sender<()>,
        )>,
    ) {
        let input_state_provider = input_state.get_provider();
        let mut context = SubStepUpdateContext::new(
            &mut self.metadata,
            input_state_provider,
            outputs_to_swallow,
            output_sender,
        );

        while context.metadata.get_next_scheduled_time().map(|s| s.time) == Some(time) {
            context.metadata.last_updated.time = time;
            context.metadata.last_updated.index += 1;
            let (t, e) = context.metadata.pop_first_event().unwrap();
            self.transposer
                .handle_scheduled(t.time, e, &mut context)
                .await;
        }
    }
}
