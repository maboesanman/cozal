use std::{pin::Pin, task::Context};

use rand::{Rng, SeedableRng};
use rand_chacha::{ChaCha12Core, rand_core::block::BlockRng};

use crate::{
    core::{
        event_state_stream::{
            iter_event_state_stream::IterEventStateStream, EventStatePoll, EventStateStream,
            EventStateStreamExt,
        },
        transposer::test::test_transposer::{HandleRecord, TestTransposer},
    },
    test::test_waker::DummyWaker,
};

#[test]
fn poll() {
    // this event state stream emits an event every 10, with payload i, and state i^2
    let test_input_iter = (0..).map(|i| (10 * i, i, i * i));
    let test_input = IterEventStateStream::new(test_input_iter, 0);

    let seed = rand::thread_rng().gen();

    let mut engine = test_input.into_engine::<_, 20>(TestTransposer::new(vec![]), seed);
    let engine_ref = &mut engine;

    let mut engine_pin = unsafe { Pin::new_unchecked(engine_ref) };

    let (waker, _) = DummyWaker::new();

    let mut cx = Context::from_waker(&waker);

    let poll = engine_pin.as_mut().poll(15, &mut cx);
    assert!(matches!(poll, EventStatePoll::Event(0, 0)));

    let poll = engine_pin.as_mut().poll(15, &mut cx);
    assert!(matches!(poll, EventStatePoll::Event(1, 10)));

    let poll = engine_pin.as_mut().poll(15, &mut cx);
    assert!(matches!(poll, EventStatePoll::Scheduled(_, 20)));

    let poll = engine_pin.as_mut().poll(35, &mut cx);
    assert!(matches!(poll, EventStatePoll::Event(4, 20)));

    let poll = engine_pin.as_mut().poll(15, &mut cx);
    assert!(matches!(poll, EventStatePoll::Scheduled(_, 30)));

    let poll = engine_pin.as_mut().poll(30, &mut cx);
    assert!(matches!(poll, EventStatePoll::Event(9, 30)));
}

#[test]
fn poll_forget() {
    // this event state stream emits an event every 10, with payload i, and state i^2
    let test_input_iter = (0..).map(|i| (10 * i, i, i * i));
    let test_input = IterEventStateStream::new(test_input_iter, 0);

    let seed = rand::thread_rng().gen();

    let mut engine = test_input.into_engine::<_, 20>(TestTransposer::new(vec![]), seed);
    let engine_ref = &mut engine;

    let mut engine_pin = unsafe { Pin::new_unchecked(engine_ref) };

    let (waker, _) = DummyWaker::new();

    let mut cx = Context::from_waker(&waker);

    let poll = engine_pin.as_mut().poll_forget(15, &mut cx);
    assert!(matches!(poll, EventStatePoll::Event(0, 0)));

    let poll = engine_pin.as_mut().poll_forget(15, &mut cx);
    assert!(matches!(poll, EventStatePoll::Event(1, 10)));

    let poll = engine_pin.as_mut().poll_forget(15, &mut cx);
    assert!(matches!(poll, EventStatePoll::Scheduled(_, 20)));

    let poll = engine_pin.as_mut().poll_forget(35, &mut cx);
    assert!(matches!(poll, EventStatePoll::Event(4, 20)));

    let poll = engine_pin.as_mut().poll_forget(15, &mut cx);
    assert!(matches!(poll, EventStatePoll::Scheduled(_, 30)));

    let poll = engine_pin.as_mut().poll_forget(30, &mut cx);
    assert!(matches!(poll, EventStatePoll::Event(9, 30)));
}

#[test]
fn ordering_invariability() {
    // this event state stream emits an event every 10, with payload i, and state i^2
    let items = [1, 3, 0, 2];
    let test_input_iter = items.iter().map(|&i| (10 * i, i, i * i));
    // let test_input_iter = (0..).map(|i| (10 * i, i, i * i));
    let test_input = IterEventStateStream::new(test_input_iter, 0);

    let seed = rand::thread_rng().gen();

    let mut engine = test_input.into_engine::<_, 20>(TestTransposer::new(vec![]), seed);
    let engine_ref = &mut engine;

    let mut engine_pin = unsafe { Pin::new_unchecked(engine_ref) };

    let (waker, _) = DummyWaker::new();

    let mut cx = Context::from_waker(&waker);

    let poll = engine_pin.as_mut().poll_events(15, &mut cx);
    assert!(matches!(poll, EventStatePoll::Event(1, 10)));

    let poll = engine_pin.as_mut().poll_events(15, &mut cx);
    assert!(matches!(poll, EventStatePoll::Scheduled((), 30)));

    loop {
        match engine_pin.as_mut().poll_events(35, &mut cx) {
            EventStatePoll::Ready(()) => break,
            _ => continue,
        }
    }

    if let EventStatePoll::Ready(state) = engine_pin.as_mut().poll(5, &mut cx) {
        if let (HandleRecord::Input(t, v), _) = &state[0] {
            assert_eq!(*t, 0);
            assert_eq!(v.len(), 1);
            assert_eq!(*v.first().unwrap(), 0);
        } else {
            panic!("not input");
        }
    } else {
        panic!("poll wasn't ready");
    }

    if let EventStatePoll::Ready(state) = engine_pin.as_mut().poll(15, &mut cx) {
        if let (HandleRecord::Input(t, v), _) = &state[0] {
            assert_eq!(*t, 0);
            assert_eq!(v.len(), 1);
            assert_eq!(*v.first().unwrap(), 0);
        } else {
            panic!("not input");
        }

        if let (HandleRecord::Input(t, v), _) = &state[1] {
            assert_eq!(*t, 10);
            assert_eq!(v.len(), 1);
            assert_eq!(*v.first().unwrap(), 1);
        } else {
            panic!("not input");
        }
    } else {
        panic!("poll wasn't ready");
    }

    if let EventStatePoll::Ready(state) = engine_pin.as_mut().poll(25, &mut cx) {
        if let (HandleRecord::Input(t, v), _) = &state[0] {
            assert_eq!(*t, 0);
            assert_eq!(v.len(), 1);
            assert_eq!(*v.first().unwrap(), 0);
        } else {
            panic!("not input");
        }

        if let (HandleRecord::Input(t, v), _) = &state[1] {
            assert_eq!(*t, 10);
            assert_eq!(v.len(), 1);
            assert_eq!(*v.first().unwrap(), 1);
        } else {
            panic!("not input");
        }

        if let (HandleRecord::Input(t, v), _) = &state[2] {
            assert_eq!(*t, 20);
            assert_eq!(v.len(), 1);
            assert_eq!(*v.first().unwrap(), 2);
        } else {
            panic!("not input");
        }
    } else {
        panic!("poll wasn't ready");
    }

    if let EventStatePoll::Ready(state) = engine_pin.as_mut().poll(35, &mut cx) {
        if let (HandleRecord::Input(t, v), _) = &state[0] {
            assert_eq!(*t, 0);
            assert_eq!(v.len(), 1);
            assert_eq!(*v.first().unwrap(), 0);
        } else {
            panic!("not input");
        }

        if let (HandleRecord::Input(t, v), _) = &state[1] {
            assert_eq!(*t, 10);
            assert_eq!(v.len(), 1);
            assert_eq!(*v.first().unwrap(), 1);
        } else {
            panic!("not input");
        }

        if let (HandleRecord::Input(t, v), _) = &state[2] {
            assert_eq!(*t, 20);
            assert_eq!(v.len(), 1);
            assert_eq!(*v.first().unwrap(), 2);
        } else {
            panic!("not input");
        }

        if let (HandleRecord::Input(t, v), _) = &state[3] {
            assert_eq!(*t, 30);
            assert_eq!(v.len(), 1);
            assert_eq!(*v.first().unwrap(), 3);
        } else {
            panic!("not input");
        }
    } else {
        panic!("poll wasn't ready");
    }
}

#[test]
fn rng() {
    // this event state stream emits an event every 10, with payload i, and state i^2
    let items = [1, 3, 0, 2];
    let test_input_iter = items.iter().map(|&i| (10 * i, i, i * i));
    // let test_input_iter = (0..).map(|i| (10 * i, i, i * i));
    let test_input = IterEventStateStream::new(test_input_iter, 0);

    let seed = rand::thread_rng().gen();

    let mut rng = BlockRng::new(ChaCha12Core::from_seed(seed));

    let mut engine = test_input.into_engine::<_, 20>(TestTransposer::new(vec![]), seed);
    let engine_ref = &mut engine;

    let mut engine_pin = unsafe { Pin::new_unchecked(engine_ref) };

    let (waker, _) = DummyWaker::new();

    let mut cx = Context::from_waker(&waker);

    let poll = engine_pin.as_mut().poll_events(15, &mut cx);
    assert!(matches!(poll, EventStatePoll::Event(1, 10)));

    let poll = engine_pin.as_mut().poll_events(15, &mut cx);
    assert!(matches!(poll, EventStatePoll::Scheduled((), 30)));

    loop {
        match engine_pin.as_mut().poll_events(35, &mut cx) {
            EventStatePoll::Ready(()) => break,
            _ => continue,
        }
    }

    if let EventStatePoll::Ready(state) = engine_pin.as_mut().poll(35, &mut cx) {
        assert_eq!(rng.gen::<u64>(), state[0].1);
        assert_eq!(rng.gen::<u64>(), state[1].1);
        assert_eq!(rng.gen::<u64>(), state[2].1);
        assert_eq!(rng.gen::<u64>(), state[3].1);
    } else {
        panic!("poll wasn't ready");
    }
}
