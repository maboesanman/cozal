use super::{transposer_frame::TransposerFrame, Transposer};

pub(super) struct TransposerHistory<T: Transposer> {
    frames: Vec<HistoryFrame<T>>,
}

struct HistoryFrame<T: Transposer> {
    pub start: TransposerFrame<T>,
    pub events: Vec<EventFrame<T>>,
}

struct EventFrame<T: Transposer> {
    pub time: T::Time,
    pub inputs: Vec<T::Input>,
    pub outputs: bool,
}

impl<T: Transposer> TransposerHistory<T> {
    pub fn new(initial_frame: TransposerFrame<T>) -> Self {
        let mut new_history = TransposerHistory { frames: Vec::new() };
        new_history.push_frame(initial_frame);
        new_history
    }

    pub fn push_events(&mut self, time: T::Time, inputs: Vec<T::Input>, outputs: bool) {
        self.frames.last_mut().unwrap().events.push(EventFrame {
            time,
            inputs,
            outputs,
        });
    }

    pub fn push_frame(&mut self, frame: TransposerFrame<T>) {
        self.frames.push(HistoryFrame {
            start: frame,
            events: Vec::new(),
        })
    }

    // pub fn revert(
    //     &mut self,
    //     time: T::Time,
    // ) -> (TransposerFrame<T>, Vec<InternalInputEvent<T>>, bool) {
    //     todo!()
    // }
}
