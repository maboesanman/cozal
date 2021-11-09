use super::super::frame::Frame;
use crate::transposer::Transposer;

pub struct UpdateResult<T: Transposer> {
    pub frame:   Box<Frame<T>>,
    pub outputs: Vec<T::Output>,
    pub inputs:  Option<Vec<T::Input>>,
}
