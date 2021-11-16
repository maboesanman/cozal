use super::{Arg, UpdateContext, WrappedTransposer};
use crate::transposer::Transposer;

pub struct UpdateResult<T: Transposer, C: UpdateContext<T>, A: Arg<T>> {
    pub frame:   Box<WrappedTransposer<T>>,
    pub outputs: C::Outputs,
    pub arg:     A::Stored,
}
