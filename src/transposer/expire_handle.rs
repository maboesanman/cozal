/// this is the handle that you use to expire scheduled events.
#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct ExpireHandle(u64);

impl ExpireHandle {
    pub(crate) fn new(value: u64) -> Self {
        ExpireHandle(value)
    }
}
