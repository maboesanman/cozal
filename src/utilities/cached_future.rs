// use std::{sync::Arc, pin::Pin};

// use futures::Future;

// struct CachedFuture<T> {
//     inner: Arc<CachedFutureInner<T>>
// }

// struct CachedFutureGuard<'a, T> {
//     inner: 
// }

// enum CachedFutureGuardState {

// }

// enum CachedFutureInner<T> {
//     Function(fn() -> Pin<Box<dyn Future<Output = T>>>),
//     Future(Pin<Box<dyn Future<Output = T>>>),
//     Value(T),
// }

// impl<T> CachedFuture<T> {
//     pub fn new(func: fn() -> Pin<Box<dyn Future<Output = T>>>) -> Self {
//         CachedFuture {
//             inner: CachedFutureInner::Function(func),
//         }
//     }

//     pub fn new_cached(value: T) -> Self {
//         CachedFuture {
//             inner: CachedFutureInner::Value(value)
//         }
//     }

//     pub fn lock<'a>(&'a self) -> CachedFutureGuard<'a, T> {

//     }
// }