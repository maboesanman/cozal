use super::super::frame::Frame;
use crate::transposer::Transposer;

pub struct UpdateResult<T: Transposer> {
    pub frame:   Frame<T>,
    pub outputs: Vec<T::Output>,
    pub inputs:  Option<Vec<T::Input>>,
    pub exit:    bool,
}
