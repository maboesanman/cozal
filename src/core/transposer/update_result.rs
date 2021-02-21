use super::{engine_context::EngineContext, Transposer};

pub(super) struct UpdateResult<T: Transposer> {
    pub outputs: Vec<T::Output>,
    pub exit: bool,
}

impl<T: Transposer> From<EngineContext<'_, T>> for UpdateResult<T> {
    fn from(used_context: EngineContext<T>) -> Self {
        let EngineContext { outputs, .. } = used_context;
        UpdateResult {
            outputs,
            exit: false,
        }
    }
}