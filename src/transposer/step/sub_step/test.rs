use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use futures_core::Future;
use matches::assert_matches;
use rand::Rng;

use crate::transposer::context::{HandleScheduleContext, InitContext, InterpolateContext};
use crate::transposer::schedule_storage::DefaultStorage;
use crate::transposer::step::lazy_state::LazyState;
use crate::transposer::step::sub_step::SubStep;
use crate::transposer::Transposer;
use crate::util::dummy_waker::DummyWaker;

#[derive(Clone, Debug)]
struct TestTransposer {
    counter: usize,
}

#[async_trait(?Send)]
impl Transposer for TestTransposer {
    type Time = usize;

    type InputState = ();

    type OutputState = ();

    type Input = ();

    type Scheduled = ();

    type Output = usize;

    async fn init(&mut self, cx: &mut dyn InitContext<'_, Self>) {
        self.counter = 0;
        cx.schedule_event(0, ()).unwrap();
    }

    async fn handle_scheduled(
        &mut self,
        time: Self::Time,
        _payload: Self::Scheduled,
        cx: &mut dyn HandleScheduleContext<'_, Self>,
    ) {
        cx.schedule_event(time + 1, ()).unwrap();

        self.counter += 1;
        cx.emit_event(self.counter * 10);
    }

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
fn saturate_take() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();
    let mut next_input = None;

    let s = Arc::new(LazyState::new());
    let mut init = SubStep::<_, DefaultStorage>::new_init(transposer, rng_seed, &s);

    let waker = DummyWaker::dummy();
    let mut cx = Context::from_waker(&waker);

    assert_matches!(Pin::new(&mut init).poll(&mut cx), Poll::Ready(Ok(None)));

    let s = Arc::new(LazyState::new());
    let mut scheduled = init;

    for i in 1..100 {
        let mut scheduled_next = scheduled
            .next_unsaturated(&mut next_input, &s)
            .unwrap()
            .unwrap();
        scheduled_next.saturate_take(&mut scheduled).unwrap();

        let poll_result = Pin::new(&mut scheduled_next).poll(&mut cx);
        if let Poll::Ready(Ok(Some(o))) = poll_result {
            assert_eq!(o.len(), 1);
            assert_eq!(*o.first().unwrap(), i * 10);
        }

        scheduled = scheduled_next;
    }
}

#[test]
fn saturate_clone() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();
    let mut next_input = None;

    let s = Arc::new(LazyState::new());
    let mut init = SubStep::<_, DefaultStorage>::new_init(transposer, rng_seed, &s);

    let waker = DummyWaker::dummy();
    let mut cx = Context::from_waker(&waker);

    assert_matches!(Pin::new(&mut init).poll(&mut cx), Poll::Ready(Ok(None)));

    let s = Arc::new(LazyState::new());
    let mut scheduled = init;

    for i in 1..100 {
        let mut scheduled_next = scheduled
            .next_unsaturated(&mut next_input, &s)
            .unwrap()
            .unwrap();
        scheduled_next.saturate_clone(&mut scheduled).unwrap();

        let poll_result = Pin::new(&mut scheduled_next).poll(&mut cx);
        if let Poll::Ready(Ok(Some(o))) = poll_result {
            assert_eq!(o.len(), 1);
            assert_eq!(*o.first().unwrap(), i * 10);
        }

        scheduled = scheduled_next;
        // s = Arc::new(LazyState::new());
    }
}

#[test]
fn desaturate() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();
    let mut next_input = None;

    let s = Arc::new(LazyState::new());
    let mut init = SubStep::<_, DefaultStorage>::new_init(transposer, rng_seed, &s);
    let waker = DummyWaker::dummy();
    let mut cx = Context::from_waker(&waker);

    let _ = Pin::new(&mut init).poll(&mut cx);

    let s = Arc::new(LazyState::new());
    let mut scheduled1 = init.next_unsaturated(&mut next_input, &s).unwrap().unwrap();
    scheduled1.saturate_clone(&mut init).unwrap();
    assert_matches!(Pin::new(&mut scheduled1).poll(&mut cx), Poll::Ready(Ok(_)));

    let s = Arc::new(LazyState::new());
    let mut scheduled2 = scheduled1
        .next_unsaturated(&mut next_input, &s)
        .unwrap()
        .unwrap();
    scheduled2.saturate_clone(&mut scheduled1).unwrap();
    assert_matches!(Pin::new(&mut scheduled2).poll(&mut cx), Poll::Ready(Ok(_)));

    scheduled1.desaturate().unwrap();
    scheduled1.saturate_clone(&mut init).unwrap();
    assert_matches!(Pin::new(&mut scheduled1).poll(&mut cx), Poll::Ready(Ok(_)));

    scheduled2.desaturate().unwrap();
    scheduled2.saturate_clone(&mut scheduled1).unwrap();
    assert_matches!(Pin::new(&mut scheduled2).poll(&mut cx), Poll::Ready(Ok(_)));

    init.desaturate().unwrap();
}

#[test]
fn next_unsaturated_same_time() {
    let transposer = TestTransposer {
        counter: 17
    };
    let rng_seed = rand::thread_rng().gen();
    let mut next_input = Some((1, vec![()].into_boxed_slice()));

    let s = Arc::new(LazyState::new());
    let mut step = SubStep::<_, DefaultStorage>::new_init(transposer, rng_seed, &s);

    let waker = DummyWaker::dummy();
    let mut cx = Context::from_waker(&waker);

    assert_matches!(Pin::new(&mut step).poll(&mut cx), Poll::Ready(Ok(None)));

    let s = Arc::new(LazyState::new());

    // let next = step.next_unsaturated(&mut next_input, &s).unwrap();

    {
        let mut next = step.next_unsaturated(&mut next_input, &s).unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        let poll_result = Pin::new(&mut next).poll(&mut cx);
        if let Poll::Ready(Ok(Some(o))) = poll_result {
            assert_eq!(o.len(), 1);
        }

        step = next;
    }
    {
        let mut next = step.next_unsaturated(&mut next_input, &s).unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        let poll_result = Pin::new(&mut next).poll(&mut cx);
        if let Poll::Ready(Ok(Some(o))) = poll_result {
            assert_eq!(o.len(), 1);
        }

        step = next;
    }
    {
        let mut next = step.next_unsaturated_same_time().unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        let poll_result = Pin::new(&mut next).poll(&mut cx);
        if let Poll::Ready(Ok(Some(o))) = poll_result {
            assert_eq!(o.len(), 1);
        }

        step = next;
    }
    {
        let mut next = step.next_unsaturated(&mut next_input, &s).unwrap().unwrap();
        next.saturate_take(&mut step).unwrap();

        let poll_result = Pin::new(&mut next).poll(&mut cx);
        if let Poll::Ready(Ok(Some(o))) = poll_result {
            assert_eq!(o.len(), 1);
        }

        step = next;
    }

    let _step = step;
}
