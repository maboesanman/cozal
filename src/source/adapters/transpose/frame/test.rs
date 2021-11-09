use async_trait::async_trait;
use rand::Rng;

use super::TransposerFrameInternal;
use crate::source::adapters::transpose::engine_time::{EngineTime, EngineTimeSchedule};
use crate::transposer::context::{
    HandleInputContext,
    HandleScheduleContext,
    InitContext,
    InterpolateContext,
};
use crate::transposer::Transposer;

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
    let init_time = EngineTime::<usize>::new_init();
    let seed = rand::thread_rng().gen();
    let mut internal = TransposerFrameInternal::<TestTransposer>::new(seed);

    assert_eq!(internal.schedule.len(), 0);
    assert_eq!(internal.expire_handles.len(), 0);

    for i in 0..100 {
        internal.schedule_event(
            EngineTimeSchedule {
                time:         10,
                parent:       init_time.clone(),
                parent_index: i,
            },
            i,
        )
    }

    assert_eq!(internal.schedule.len(), 100);
    assert_eq!(internal.expire_handles.len(), 0);

    let mut exp_handles = Vec::new();

    for i in 0..100 {
        exp_handles.push((
            internal.schedule_event_expireable(
                EngineTimeSchedule {
                    time:         20,
                    parent:       init_time.clone(),
                    parent_index: 100 + i,
                },
                i,
            ),
            i,
        ))
    }

    assert_eq!(internal.schedule.len(), 200);
    assert_eq!(internal.expire_handles.len(), 100);

    for i in 0..100 {
        internal.schedule_event(
            EngineTimeSchedule {
                time:         5 + i,
                parent:       init_time.clone(),
                parent_index: 200 + i,
            },
            i,
        )
    }

    assert_eq!(internal.schedule.len(), 300);
    assert_eq!(internal.expire_handles.len(), 100);

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
    assert_eq!(internal.expire_handles.len(), 0);
}
