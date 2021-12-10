// use std::pin::Pin;
// use std::task::Waker;

// use futures_core::Future;

// use super::step::LazyState;
// use crate::transposer::Transposer;

// pub struct Interpolation<T: Transposer> {
//     inner: InterpolationInner<T>,
//     state: LazyState<T::InputState>,
// }

// enum InterpolationInner<T: Transposer> {
//     Pending {
//         base_time: T::Time,
//         time:      T::Time,
//     },
//     Active {
//         future: Box<dyn Future<Output = T::OutputState>>,
//         time:   T::Time,
//         waker:  Waker,
//     },
// }

// impl<T: Transposer> Interpolation<T> {
//     pub fn wake(&self) {
//         if let InterpolationInner::Active {
//             waker, ..
//         } = &self.inner
//         {
//             waker.wake_by_ref()
//         }
//     }

//     pub fn time(&self) -> T::Time {
//         match self.inner {
//             InterpolationInner::Pending {
//                 time, ..
//             } => time,
//             InterpolationInner::Active {
//                 time, ..
//             } => time,
//         }
//     }

//     pub fn new(&base_time: T::Time, time: T::Time) -> Self {
//         Self {
//             state: LazyState::new(),
//             inner: InterpolationInner::Pending {
//                 base_time,
//                 time,
//             },
//         }
//     }

//     pub fn poll(self: Pin<&mut Self>, waker: Waker) {
//         let this = unsafe { self.get_unchecked_mut() };
//         if let InterpolationInner::Pending {
//             base_time,
//             time,
//         } = this.inner
//         {
//             let state = &mut this.state;
//             let future = T::interpolate(&self, base_time, interpolated_time, cx)
//             this.inner = InterpolationInner::Active {
//                 future: Box::new
//             }
//         }
//     }
// }

// pub enum InterpolatePoll<T: Transposer> {
//     Pending,
//     NeedsState,
//     Ready(T::OutputState),
// }
