use std::panic;

pub fn take_or_recover<T, F, R>(mut_ref: &mut T, recover: R, closure: F)
where
    F: FnOnce(T) -> T,
    R: FnOnce() -> T,
{
    unsafe {
        let old_t = core::ptr::read(mut_ref);
        let new_t = panic::catch_unwind(panic::AssertUnwindSafe(|| closure(old_t)));
        match new_t {
            Err(err) => {
                let r = panic::catch_unwind(panic::AssertUnwindSafe(recover))
                    .unwrap_or_else(|_| std::process::abort());
                core::ptr::write(mut_ref, r);
                panic::resume_unwind(err);
            },
            Ok(new_t) => core::ptr::write(mut_ref, new_t),
        }
    }
}

pub fn take_and_return_or_recover<T, V, F, R>(mut_ref: &mut T, recover: R, closure: F) -> V
where
    F: FnOnce(T) -> (T, V),
    R: FnOnce() -> T,
{
    unsafe {
        let old_t = core::ptr::read(mut_ref);
        let new_t_v = panic::catch_unwind(panic::AssertUnwindSafe(|| closure(old_t)));
        match new_t_v {
            Err(err) => {
                let r = panic::catch_unwind(panic::AssertUnwindSafe(recover))
                    .unwrap_or_else(|_| std::process::abort());
                core::ptr::write(mut_ref, r);
                panic::resume_unwind(err);
            },
            Ok((new_t, v)) => {
                core::ptr::write(mut_ref, new_t);
                v
            },
        }
    }
}
