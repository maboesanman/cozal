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
        let frame = self.frames.last_mut().unwrap();
        frame.events.push(EventFrame {
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

    pub fn revert(
        &mut self,
        time: T::Time,
    ) -> (TransposerFrame<T>, Vec<(T::Time, Vec<T::Input>)>, bool) {
        if time == T::Time::default() {
            panic!();
        }

        let mut events_emitted = false;
        let mut replay_events = Vec::new();

        while let Some(top) = self.frames.pop() {
            if top.start.time < time {
                self.frames.push(top);
                break;
            }

            for event in top.events {
                events_emitted |= event.outputs;
                replay_events.push((event.time, event.inputs));
            }
        }
        if let Some(top) = self.frames.last_mut() {
            while let Some(event_frame) = top.events.pop() {
                events_emitted |= event_frame.outputs;
                replay_events.push((event_frame.time, event_frame.inputs));
            }
            (top.start.clone(), replay_events, events_emitted)
        } else {
            unreachable!();
        }
    }
}
