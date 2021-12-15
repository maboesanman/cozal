use super::{Arg, UpdateContext, WrappedTransposer};
use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::Transposer;

pub struct UpdateResult<T: Transposer, S: StorageFamily, C: UpdateContext<T, S>, A: Arg<T, S>> {
    pub wrapped_transposer: Box<WrappedTransposer<T, S>>,
    pub outputs:            C::Outputs,
    pub arg:                A::Stored,
}
