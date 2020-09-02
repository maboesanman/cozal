use super::expire_handle::{ExpireHandle, ExpireHandleFactory};
use std::collections::HashMap;
/// This is passed to [`Transposer`](super::transposer::Transposer)'s
/// [`init`](super::transposer::Transposer::init) and [`update`](super::transposer::Transposer::update) functions,
/// enabling the following in a deterministic, rollback enabled way:
/// - expire events, via [`get_expire_handle`](TransposerContext::get_expire_handle)
/// - TODO: get external state
/// - TODO: generate random values
pub struct TransposerContext {
    // this is really an AtomicNonZeroU64
    pub(super) expire_handle_factory: ExpireHandleFactory,
    pub(super) new_expire_handles: HashMap<usize, ExpireHandle>,
    // todo add seeded deterministic random function
}

#[allow(dead_code)]
impl TransposerContext {
    pub(super) fn new(handle_factory: ExpireHandleFactory) -> Self {
        Self {
            expire_handle_factory: handle_factory,
            new_expire_handles: HashMap::new(),
        }
    }
    /// get a handle that can be used to expire an event that has been scheduled.
    /// the index argument is the index in the [`InitResult::new_events`](super::transposer::InitResult::new_events)
    /// or [`UpdateResult::new_events`](super::transposer::UpdateResult::new_events)
    /// array that the handle will correspond to.
    ///
    /// These handles are meant to be stored (either in the transposer or in a scheduled event payload)
    /// until they are returned via the [`UpdateResult::expired_events`](super::transposer::UpdateResult::expired_events),
    /// which will trigger the transposer engine to remove the associated events.
    ///
    /// You can only generate a handle for an event in the same update that creates the event in the first place,
    /// so you need to create them in advance or you will not be able to interact with that event until it actually
    /// occurs.
    pub fn get_expire_handle(&mut self, index: usize) -> u64 {
        let handles = &mut self.new_expire_handles;

        if handles.get(&index).is_none() {
            let handle = self.expire_handle_factory.next();
            handles.insert(index, handle);
        }

        handles[&index].get()
    }

    pub(super) fn get_current_expire_handle(self) -> ExpireHandleFactory {
        self.expire_handle_factory
    }

    // todo add functions to get state from other streams somehow...
}
