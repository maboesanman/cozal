use async_trait::async_trait;
use rand::Rng;

use super::super::super::time::ScheduledTime;
use super::{TransposerMetaData, WrappedTransposer};
use crate::transposer::context::InterpolateContext;
use crate::transposer::schedule_storage::ImArcStorage;
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

    async fn interpolate(
        &self,
        _base_time: Self::Time,
        _interpolated_time: Self::Time,
        _cx: &mut dyn InterpolateContext<'_, Self>,
    ) -> Self::OutputState {
        unimplemented!()
    }
}

#[test]
fn metadata() {
    let seed = rand::thread_rng().gen();
    let mut metadata = TransposerMetaData::<TestTransposer, ImArcStorage>::new(seed);

    assert_eq!(metadata.schedule.len(), 0);
    assert_eq!(metadata.expire_handles_forward.len(), 0);
    assert_eq!(metadata.expire_handles_backward.len(), 0);

    for i in 0..100 {
        metadata.schedule_event(
            ScheduledTime {
                time:           10,
                parent_index:   0,
                emission_index: i,
            },
            i,
        )
    }

    assert_eq!(metadata.schedule.len(), 100);
    assert_eq!(metadata.expire_handles_forward.len(), 0);
    assert_eq!(metadata.expire_handles_backward.len(), 0);

    let mut exp_handles = Vec::new();

    for i in 0..100 {
        exp_handles.push((
            metadata.schedule_event_expireable(
                ScheduledTime {
                    time:           20,
                    parent_index:   0,
                    emission_index: 100 + i,
                },
                i,
            ),
            i,
        ))
    }

    assert_eq!(metadata.schedule.len(), 200);
    assert_eq!(metadata.expire_handles_forward.len(), 100);
    assert_eq!(metadata.expire_handles_backward.len(), 100);

    for i in 0..100 {
        metadata.schedule_event(
            ScheduledTime {
                time:           5 + i,
                parent_index:   0,
                emission_index: 200 + i,
            },
            i,
        )
    }

    assert_eq!(metadata.schedule.len(), 300);
    assert_eq!(metadata.expire_handles_forward.len(), 100);
    assert_eq!(metadata.expire_handles_backward.len(), 100);

    for (handle, payload) in exp_handles {
        let r = metadata.expire_event(handle);
        if let Ok((t, p)) = r {
            assert_eq!(t, 20);
            assert_eq!(p, payload);
        } else {
            panic!("expiration error");
        }
    }

    assert_eq!(metadata.schedule.len(), 200);
    assert_eq!(metadata.expire_handles_forward.len(), 0);
    assert_eq!(metadata.expire_handles_backward.len(), 0);
}

#[test]
fn metadata_pop() {
    let seed = rand::thread_rng().gen();
    let mut metadata = TransposerMetaData::<TestTransposer, ImArcStorage>::new(seed);

    assert_eq!(metadata.schedule.len(), 0);
    assert_eq!(metadata.expire_handles_forward.len(), 0);
    assert_eq!(metadata.expire_handles_backward.len(), 0);

    metadata.schedule_event(
        ScheduledTime {
            time:           10,
            parent_index:   0,
            emission_index: 0,
        },
        17,
    );

    assert_eq!(metadata.schedule.len(), 1);
    assert_eq!(metadata.expire_handles_forward.len(), 0);
    assert_eq!(metadata.expire_handles_backward.len(), 0);

    metadata.schedule_event_expireable(
        ScheduledTime {
            time:           20,
            parent_index:   0,
            emission_index: 1,
        },
        23,
    );

    assert_eq!(metadata.schedule.len(), 2);
    assert_eq!(metadata.expire_handles_forward.len(), 1);
    assert_eq!(metadata.expire_handles_backward.len(), 1);

    assert_eq!(metadata.get_next_scheduled_time().unwrap().time, 10);
    assert_eq!(metadata.pop_first_event().unwrap().1, 17);

    assert_eq!(metadata.schedule.len(), 1);
    assert_eq!(metadata.expire_handles_forward.len(), 1);
    assert_eq!(metadata.expire_handles_backward.len(), 1);

    assert_eq!(metadata.get_next_scheduled_time().unwrap().time, 20);
    assert_eq!(metadata.pop_first_event().unwrap().1, 23);

    assert_eq!(metadata.schedule.len(), 0);
    assert_eq!(metadata.expire_handles_forward.len(), 0);
    assert_eq!(metadata.expire_handles_backward.len(), 0);

    assert!(metadata.get_next_scheduled_time().is_none());
    assert!(metadata.pop_first_event().is_none());
}

#[test]
fn metadata_failed_expire() {
    let seed = rand::thread_rng().gen();
    let mut metadata = TransposerMetaData::<TestTransposer, ImArcStorage>::new(seed);

    let handle = metadata.schedule_event_expireable(
        ScheduledTime {
            time:           10,
            parent_index:   0,
            emission_index: 0,
        },
        17,
    );

    assert!(metadata.pop_first_event().is_some());

    assert!(metadata.expire_event(handle).is_err());
}

#[test]
fn wrapped_transposer() {
    let transposer = TestTransposer {};
    let seed = rand::thread_rng().gen();
    let mut wrapped_transposer = WrappedTransposer::<_, ImArcStorage>::new(transposer, seed);

    assert!(wrapped_transposer.get_next_scheduled_time().is_none());
    assert!(wrapped_transposer.pop_schedule_event().is_none());

    for i in 3..10 {
        wrapped_transposer.metadata.schedule_event(
            ScheduledTime {
                time:           5 + i,
                parent_index:   0,
                emission_index: i,
            },
            i,
        )
    }

    assert_eq!(
        wrapped_transposer.get_next_scheduled_time().unwrap().time,
        8
    );
    let (t, p) = wrapped_transposer.pop_schedule_event().unwrap();
    assert_eq!(t.time, 8);
    assert_eq!(p, 3);
}

#[test]
fn wrapped_transposer_clone() {
    let transposer = TestTransposer {};
    let seed = rand::thread_rng().gen();
    let mut wrapped_transposer = WrappedTransposer::<_, ImArcStorage>::new(transposer, seed);

    assert!(wrapped_transposer.get_next_scheduled_time().is_none());
    assert!(wrapped_transposer.pop_schedule_event().is_none());

    for i in 3..10 {
        wrapped_transposer.metadata.schedule_event(
            ScheduledTime {
                time:           5 + i,
                parent_index:   0,
                emission_index: i,
            },
            i,
        )
    }

    let mut wrapped_transposer_clone = wrapped_transposer.clone();

    assert_eq!(
        wrapped_transposer.get_next_scheduled_time().unwrap().time,
        8
    );
    let (t, p) = wrapped_transposer.pop_schedule_event().unwrap();
    assert_eq!(t.time, 8);
    assert_eq!(p, 3);

    assert_eq!(
        wrapped_transposer_clone
            .get_next_scheduled_time()
            .unwrap()
            .time,
        8
    );
    let (t, p) = wrapped_transposer_clone.pop_schedule_event().unwrap();
    assert_eq!(t.time, 8);
    assert_eq!(p, 3);
}

#[test]
#[should_panic]
fn metadata_expire_illegal() {
    let seed = rand::thread_rng().gen();
    let mut metadata = TransposerMetaData::<TestTransposer, ImArcStorage>::new(seed);

    let handle = metadata.schedule_event_expireable(
        ScheduledTime {
            time:           20,
            parent_index:   0,
            emission_index: 1,
        },
        23,
    );

    let time = metadata.expire_handles_forward.get(&handle).unwrap();

    // breaks invariants. very illegal
    metadata.schedule.remove(&time);

    let _ = metadata.expire_event(handle);
}
