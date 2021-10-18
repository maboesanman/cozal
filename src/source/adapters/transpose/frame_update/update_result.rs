use super::super::transposer_frame::TransposerFrame;
use crate::transposer::Transposer;

pub struct UpdateResult<T: Transposer> {
    pub frame:   TransposerFrame<T>,
    pub outputs: Vec<T::Output>,
    pub inputs:  Option<Vec<T::Input>>,
    pub exit:    bool,
}
