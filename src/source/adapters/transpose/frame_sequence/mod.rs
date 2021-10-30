use std::collections::VecDeque;

use self::frame_sequence_item::FrameSequenceItem;
use crate::transposer::Transposer;

pub(self) mod frame_sequence_item;

pub struct FrameSequence<T: Transposer> {
    inner: VecDeque<FrameSequenceItem<T>>,
}
