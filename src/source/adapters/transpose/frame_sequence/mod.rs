use std::collections::VecDeque;

use super::sequence_frame_update::SequenceFrameUpdate;
use crate::transposer::Transposer;

pub struct FrameSequence<T: Transposer> {
    inner: VecDeque<SequenceFrameUpdate<T>>,
}
