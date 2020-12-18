use std::{pin::Pin, task::Context};

use crate::{core::{Event, transposer::{transposer_frame::TransposerFrame, transposer_update::TransposerUpdate}}, test::test_waker::DummyWaker};

use super::test_transposer::{EventCall, TestTransposer};

use crate::core::transposer::transposer_update::TransposerUpdatePoll;

#[test]
fn test_input() {
    let transposer = TestTransposer::new(vec![]);
    let frame = TransposerFrame::new(transposer);
    let time = 12;
    let inputs = vec![7,6,5];
    let state = 17;
    let update = TransposerUpdate::new_input(frame, time, inputs, state);
    // let update_ref = unsafe { Pin::new_unchecked(&mut update) };
    let mut pinned = update.init();

    let (waker, _) = DummyWaker::new();
    let mut context = Context::from_waker(&waker);
    let result = pinned.as_mut().poll(None, &mut context);
    
    match result {
        TransposerUpdatePoll::Ready(ready_result) => {
            assert_eq!(ready_result.inputs, Some(vec![7,6,5]));
            assert_eq!(ready_result.input_state, Some(17));

            let result = ready_result.result;

            assert_eq!(result.output_events.first().unwrap().payload, vec![
                            EventCall::Input(7),
                            EventCall::Input(6),
                            EventCall::Input(5)
                        ]);
            let frame = result.frame;

            assert_eq!(frame.internal.schedule.len(), 2);
        }
        _ => assert!(false, "result not ready")
    }
}
