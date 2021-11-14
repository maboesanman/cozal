use crate::transposer::ExpireHandle;

#[derive(Clone, Debug)]
pub struct ExpireHandleFactory(u64);

impl ExpireHandleFactory {
    pub fn new() -> Self {
        ExpireHandleFactory(0)
    }

    pub fn next(&mut self) -> ExpireHandle {
        let handle = ExpireHandle::new(self.0);
        self.0 += 1;
        handle
    }
}
