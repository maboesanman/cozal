use super::{context::UpdateContext, transposer_frame::TransposerFrame, Transposer};

pub(super) struct WrappedUpdateResult<T: Transposer> {
    pub frame: TransposerFrame<T>,
    pub outputs: Vec<T::Output>,
    pub exit: bool,
}

impl<T: Transposer> WrappedUpdateResult<T> {
    pub fn new<'a>(mutated_frame: TransposerFrame<T>, used_context: UpdateContext<T>) -> Self {
        let UpdateContext { outputs, exit, .. } = used_context;
        WrappedUpdateResult {
            frame: mutated_frame,
            outputs,
            exit,
        }
    }
}
