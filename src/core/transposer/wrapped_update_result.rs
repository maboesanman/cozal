use super::{
    context::UpdateContext, transposer_frame::TransposerFrame, InternalOutputEvent, Transposer,
};

pub(super) struct WrappedUpdateResult<T: Transposer> {
    pub frame: TransposerFrame<T>,
    pub output_events: Vec<InternalOutputEvent<T>>,
    pub exit: bool,
}

impl<T: Transposer> WrappedUpdateResult<T> {
    pub fn new<'a>(mutated_frame: TransposerFrame<T>, used_context: UpdateContext<'a, T>) -> Self {
        let UpdateContext {
            output_events,
            exit,
            ..
        } = used_context;
        WrappedUpdateResult {
            frame: mutated_frame,
            output_events,
            exit,
        }
    }
}
