use super::super::frame::Frame;
use super::arg::Arg;
use crate::transposer::Transposer;

pub struct UpdateResult<T: Transposer, A: Arg<T>> {
    pub frame:   Box<Frame<T>>,
    pub outputs: Vec<T::Output>,
    pub arg:     A::Stored,
}
