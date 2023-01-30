use crate::expire_handle::ExpireHandle;

#[derive(Clone, Debug, Default)]
pub struct ExpireHandleFactory(u64);

impl ExpireHandleFactory {
    pub fn next(&mut self) -> ExpireHandle {
        let handle = ExpireHandle::new(self.0);
        self.0 += 1;
        handle
    }
}
