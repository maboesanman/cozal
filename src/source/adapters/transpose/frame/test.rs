use async_trait::async_trait;
use rand::Rng;

use super::{Frame, FrameMetaData};
use crate::source::adapters::transpose::engine_time::{EngineTime, EngineTimeSchedule};
use crate::transposer::context::{
    HandleInputContext,
    HandleScheduleContext,
    InitContext,
    InterpolateContext,
};
use crate::transposer::Transposer;

#[derive(Clone)]
struct TestTransposer {}

#[async_trait(?Send)]
impl Transposer for TestTransposer {
    type Time = usize;

    type InputState = ();

    type OutputState = ();

    type Input = ();

    type Scheduled = usize;

    type Output = ();

    async fn init(&mut self, _cx: &mut dyn InitContext<Self>) {}

    async fn handle_input(
        &mut self,
        _time: Self::Time,
        _inputs: &[Self::Input],
        _cx: &mut dyn HandleInputContext<Self>,
    ) {
    }

    async fn handle_scheduled(
        &mut self,
        _time: Self::Time,
        _payload: Self::Scheduled,
        _cx: &mut dyn HandleScheduleContext<Self>,
    ) {
    }

    async fn interpolate(
        &self,
        _base_time: Self::Time,
        _interpolated_time: Self::Time,
        _state: Self::InputState,
        _cx: &mut dyn InterpolateContext<Self>,
    ) -> Self::OutputState {
        unimplemented!()
    }
}

#[test]
fn frame_internal() {
    let seed = rand::thread_rng().gen();
    let mut internal = FrameMetaData::<TestTransposer>::new(seed);

    assert_eq!(internal.schedule.len(), 0);
    assert_eq!(internal.expire_handles_forward.len(), 0);
    assert_eq!(internal.expire_handles_backward.len(), 0);

    for i in 0..100 {
        internal.schedule_event(
            EngineTimeSchedule {
                time:           10,
                parent_index:   0,
                emission_index: i,
            },
            i,
        )
    }

    assert_eq!(internal.schedule.len(), 100);
    assert_eq!(internal.expire_handles_forward.len(), 0);
    assert_eq!(internal.expire_handles_backward.len(), 0);

    let mut exp_handles = Vec::new();

    for i in 0..100 {
        exp_handles.push((
            internal.schedule_event_expireable(
                EngineTimeSchedule {
                    time:           20,
                    parent_index:   0,
                    emission_index: 100 + i,
                },
                i,
            ),
            i,
        ))
    }

    assert_eq!(internal.schedule.len(), 200);
    assert_eq!(internal.expire_handles_forward.len(), 100);
    assert_eq!(internal.expire_handles_backward.len(), 100);

    for i in 0..100 {
        internal.schedule_event(
            EngineTimeSchedule {
                time:           5 + i,
                parent_index:   0,
                emission_index: 200 + i,
            },
            i,
        )
    }

    assert_eq!(internal.schedule.len(), 300);
    assert_eq!(internal.expire_handles_forward.len(), 100);
    assert_eq!(internal.expire_handles_backward.len(), 100);

    for (handle, payload) in exp_handles {
        let r = internal.expire_event(handle);
        if let Ok((t, p)) = r {
            assert_eq!(t, 20);
            assert_eq!(p, payload);
        } else {
            panic!("expiration error");
        }
    }

    assert_eq!(internal.schedule.len(), 200);
    assert_eq!(internal.expire_handles_forward.len(), 0);
    assert_eq!(internal.expire_handles_backward.len(), 0);
}

#[test]
fn frame_internal_pop() {
    let init_time = EngineTime::<usize>::new_init();
    let seed = rand::thread_rng().gen();
    let mut internal = FrameMetaData::<TestTransposer>::new(seed);

    assert_eq!(internal.schedule.len(), 0);
    assert_eq!(internal.expire_handles_forward.len(), 0);
    assert_eq!(internal.expire_handles_backward.len(), 0);

    internal.schedule_event(
        EngineTimeSchedule {
            time:           10,
            parent_index:   0,
            emission_index: 0,
        },
        17,
    );

    assert_eq!(internal.schedule.len(), 1);
    assert_eq!(internal.expire_handles_forward.len(), 0);
    assert_eq!(internal.expire_handles_backward.len(), 0);

    internal.schedule_event_expireable(
        EngineTimeSchedule {
            time:           20,
            parent_index:   0,
            emission_index: 1,
        },
        23,
    );

    assert_eq!(internal.schedule.len(), 2);
    assert_eq!(internal.expire_handles_forward.len(), 1);
    assert_eq!(internal.expire_handles_backward.len(), 1);

    assert_eq!(internal.get_next_scheduled_time().unwrap().time, 10);
    assert_eq!(internal.pop_first_event().unwrap().1, 17);

    assert_eq!(internal.schedule.len(), 1);
    assert_eq!(internal.expire_handles_forward.len(), 1);
    assert_eq!(internal.expire_handles_backward.len(), 1);

    assert_eq!(internal.get_next_scheduled_time().unwrap().time, 20);
    assert_eq!(internal.pop_first_event().unwrap().1, 23);

    assert_eq!(internal.schedule.len(), 0);
    assert_eq!(internal.expire_handles_forward.len(), 0);
    assert_eq!(internal.expire_handles_backward.len(), 0);

    assert!(internal.get_next_scheduled_time().is_none());
    assert!(internal.pop_first_event().is_none());
}

#[test]
fn frame_internal_failed_expire() {
    let init_time = EngineTime::<usize>::new_init();
    let seed = rand::thread_rng().gen();
    let mut internal = FrameMetaData::<TestTransposer>::new(seed);

    let handle = internal.schedule_event_expireable(
        EngineTimeSchedule {
            time:           10,
            parent_index:   0,
            emission_index: 0,
        },
        17,
    );

    assert!(internal.pop_first_event().is_some());

    assert!(internal.expire_event(handle).is_err());
}

#[test]
fn frame() {
    let init_time = EngineTime::<usize>::new_init();
    let transposer = TestTransposer {};
    let seed = rand::thread_rng().gen();
    let mut frame = Frame::new(transposer, seed);

    assert!(frame.get_next_scheduled_time().is_none());
    assert!(frame.pop_schedule_event().is_none());

    for i in 3..10 {
        frame.metadata.schedule_event(
            EngineTimeSchedule {
                time:           5 + i,
                parent_index:   0,
                emission_index: i,
            },
            i,
        )
    }

    assert_eq!(frame.get_next_scheduled_time().unwrap().time, 8);
    let (t, p) = frame.pop_schedule_event().unwrap();
    assert_eq!(t.time, 8);
    assert_eq!(p, 3);
}

#[test]
fn frame_clone() {
    let init_time = EngineTime::<usize>::new_init();
    let transposer = TestTransposer {};
    let seed = rand::thread_rng().gen();
    let mut frame = Frame::new(transposer, seed);

    assert!(frame.get_next_scheduled_time().is_none());
    assert!(frame.pop_schedule_event().is_none());

    for i in 3..10 {
        frame.metadata.schedule_event(
            EngineTimeSchedule {
                time:           5 + i,
                parent_index:   0,
                emission_index: i,
            },
            i,
        )
    }

    let mut frame_clone = frame.clone();

    assert_eq!(frame.get_next_scheduled_time().unwrap().time, 8);
    let (t, p) = frame.pop_schedule_event().unwrap();
    assert_eq!(t.time, 8);
    assert_eq!(p, 3);

    assert_eq!(frame_clone.get_next_scheduled_time().unwrap().time, 8);
    let (t, p) = frame_clone.pop_schedule_event().unwrap();
    assert_eq!(t.time, 8);
    assert_eq!(p, 3);
}

#[test]
#[should_panic]
fn frame_internal_expire_illegal() {
    let init_time = EngineTime::<usize>::new_init();
    let seed = rand::thread_rng().gen();
    let mut internal = FrameMetaData::<TestTransposer>::new(seed);

    let handle = internal.schedule_event_expireable(
        EngineTimeSchedule {
            time:           20,
            parent_index:   0,
            emission_index: 1,
        },
        23,
    );

    let time = internal.expire_handles_forward.get(&handle).unwrap();

    // breaks invariants. very illegal
    internal.schedule.remove(&time);

    let _ = internal.expire_event(handle);
}
