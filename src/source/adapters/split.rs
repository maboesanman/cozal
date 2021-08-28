use core::pin::Pin;
use core::task::{Context, Waker};
use std::sync::{Arc, RwLock};

use crate::source::traits::SourceContext;
use crate::source::{Source, SourcePoll};

pub struct Split<const CHANNELS: usize, Src: Source<CHANNELS>, E, ConvertFn>
where
    ConvertFn: Fn(Src::Event) -> E,
{
    inner: Arc<RwLock<SplitInner<CHANNELS, Src>>>,
    convert: ConvertFn,
    index: usize,
}

pub struct SplitInner<const CHANNELS: usize, Src: Source<CHANNELS>> {
    input_source: Src,
    deciders: Vec<(fn(&Src::Event) -> bool, Option<Waker>)>,
}

impl<const CHANNELS: usize, Src: Source<CHANNELS>, E, ConvertFn> Split<CHANNELS, Src, E, ConvertFn>
where
    ConvertFn: Fn(Src::Event) -> E,
{
    pub fn new(source: Src, decide: fn(&Src::Event) -> bool, convert: ConvertFn) -> Self {
        let inner = SplitInner {
            input_source: source,
            deciders: vec![(decide, None)],
        };
        let inner = Arc::new(RwLock::new(inner));

        Self {
            inner,
            convert,
            index: 0,
        }
    }

    pub fn split(&self, decide: fn(&Src::Event) -> bool, convert: ConvertFn) -> Self {
        let inner = self.inner.clone();

        let mut lock = inner.write().unwrap();

        let index = lock.deciders.len();
        lock.deciders.push((decide, None));
        core::mem::drop(lock);

        Self {
            inner,
            convert,
            index,
        }
    }
}

impl<const CHANNELS: usize, Src: Source<CHANNELS>, E, ConvertFn> Source<CHANNELS> for Split<CHANNELS, Src, E, ConvertFn>
where
    ConvertFn: Fn(Src::Event) -> E,
{
    type Time = Src::Time;

    type Event = E;

    type State = Src::State;

    fn poll(
        self: Pin<&mut Self>,
        _time: Self::Time,
        _cx: &mut SourceContext<'_, CHANNELS, Src::Time>,
    ) -> SourcePoll<Self::Time, Self::Event, Self::State> {
        unimplemented!()
    }
}
