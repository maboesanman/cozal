use super::{InitContext, Transposer, context::UpdateContext, transposer_frame::TransposerFrame};

pub(super) struct UpdateResult<T: Transposer> {
    pub frame: TransposerFrame<T>,
    pub outputs: Vec<T::Output>,
    pub exit: bool,
}

impl<T: Transposer> UpdateResult<T> {
    pub fn from_init_context<'a>(mutated_frame: TransposerFrame<T>, used_context: InitContext<T>) -> Self {
        let InitContext { outputs, .. } = used_context;
        UpdateResult {
            frame: mutated_frame,
            outputs,
            exit: false,
        }
    }
    pub fn from_update_context<'a>(mutated_frame: TransposerFrame<T>, used_context: UpdateContext<T>) -> Self {
        let UpdateContext { outputs, exit, .. } = used_context;
        UpdateResult {
            frame: mutated_frame,
            outputs,
            exit,
        }
    }
}
