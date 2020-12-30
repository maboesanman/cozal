use std::{pin::Pin, task::Context};

use crate::{
    core::transposer::{transposer_frame::TransposerFrame, transposer_update::TransposerUpdate},
    test::test_waker::DummyWaker,
};

use super::test_transposer::{HandleRecord, TestTransposer};

use crate::core::transposer::transposer_update::TransposerUpdatePoll;

#[test]
fn test_input() {
    let transposer = TestTransposer::new(vec![]);
    let frame = TransposerFrame::new(transposer);
    let time = 12;
    let inputs = vec![7, 6, 5];
    let state = 17;
    let mut update = TransposerUpdate::new_input(frame, time, inputs, state);
    let mut update_ref = unsafe { Pin::new_unchecked(&mut update) };
    update_ref.as_mut().init_pinned();

    let (waker, _) = DummyWaker::new();
    let mut cx = Context::from_waker(&waker);
    let result = update_ref.poll(&mut cx);

    match result {
        TransposerUpdatePoll::Ready(ready_result) => {
            assert_eq!(ready_result.inputs, Some(vec![7, 6, 5]));
            assert_eq!(ready_result.input_state, Some(17));

            let mut result = ready_result.result;

            let frame = result.frame;
            assert_eq!(result.outputs.len(), 1);
            let output = result.outputs.pop().unwrap();

            assert_eq!(output, (17, HandleRecord::Input(12, vec![7, 6, 5])));

            let transposer = frame.transposer;

            assert_eq!(transposer.handle_record.len(), 1);
        }
        _ => assert!(false, "result not ready"),
    }
}
