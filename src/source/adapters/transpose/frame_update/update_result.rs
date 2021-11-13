use super::super::frame::Frame;
use super::arg::Arg;
use super::update_context::UpdateContext;
use crate::transposer::Transposer;

pub struct UpdateResult<T: Transposer, C: UpdateContext<T>, A: Arg<T>> {
    pub frame:   Box<Frame<T>>,
    pub outputs: C::Outputs,
    pub arg:     A::Stored,
}
