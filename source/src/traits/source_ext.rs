use super::Source;
use crate::adapters::MutexSource;

impl<S> SourceExt for S where S: Source {}

pub trait SourceExt: Source + Sized {
    /// Adapter for converting a schedule_stream into one that yields the items in realtime, and a struct which can be queried yielding
    ///
    /// a reference must be given for the conversion from [`S::Time`](super::schedule_stream::ScheduleStream::Time) to [`Instant`](std::time::Instant).
    /// for example if you use [`Duration`](core::time::Duration) as your time, a "start time" must be given
    /// so that the duration can be added to something.
    ///
    /// if your timestamp is an [`Instant`](std::time::Instant), then your reference is of type `()` because instants
    /// are already realtime and need no reference.
    ///
    /// Additionally, a sleep_fn must be provided. This should be simply `tokio::time::sleep_until` if using tokio, but is left generic to avoid requiring a runtime.
    // fn realtime<SleepFut: Future, SleepFn: Fn(Instant) -> SleepFut>(
    //     self,
    //     reference: <Self::Time as Timestamp>::Reference,
    //     sleep_fn: SleepFn,
    // ) -> (RealtimeEvents<Self>, RealtimeStates<Self>)
    // where
    //     Self::Time: Timestamp,
    // {
    //     realtime(self, reference, sleep_fn)
    // }

    /// Adapter for converting a source into another via a transposer.
    // fn transpose<T>(self, initial: T, rng_seed: [u8; 32]) -> Transpose<Self, T>
    // where
    //     T::Time: Copy + Ord + Default + Unpin, // TODO remove once https://github.com/rust-lang/rust/issues/91985 is resolved.
    //     T: Transposer<Time = Self::Time, Input = Self::Event, InputState = Self::State>,
    //     T: Clone,
    // {
    //     Transpose::new(self, initial, rng_seed)
    // }

    /// Adapter for offloading work to a future
    // fn offload(self) -> (OffloadSource<Self>, OffloadFuture<Self>) {
    //     offload(self)
    // }

    /// Adapter for calling a limited-channel source on any number of channels
    // fn multiplex(self) -> Multiplex<Self> {
    //     Multiplex::new(self)
    // }

    fn concurrent(self) -> MutexSource<Self> {
        MutexSource::new(self)
    }
}
