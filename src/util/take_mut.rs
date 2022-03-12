use std::panic;

/// use closure to take value from mut_ref, and supply a replacement.
/// if closure panics, use recover to fill the mut_ref, then continue unwinding
/// if recover panics, the whole process aborts.
pub fn take_or_recover<T, F, R>(mut_ref: &mut T, recover: R, closure: F)
where
    F: FnOnce(T) -> T,
    R: FnOnce() -> T,
{
    take_and_return_or_recover(mut_ref, recover, |t| (closure(t), ()))
}

/// use closure to take value from mut_ref, and supply a replacement, as well as a value to return.
/// if closure panics, use recover to fill the mut_ref, then continue unwinding
/// if recover panics, the whole process aborts.
pub fn take_and_return_or_recover<T, V, F, R>(mut_ref: &mut T, recover: R, closure: F) -> V
where
    F: FnOnce(T) -> (T, V),
    R: FnOnce() -> T,
{
    // SAFETY: this value will be replaced in all branches of this call, even when panicing in F.
    let old_t = unsafe { core::ptr::read(mut_ref) };

    let new_t_v = panic::catch_unwind(panic::AssertUnwindSafe(|| closure(old_t)));
    match new_t_v {
        Err(err) => {
            let r = panic::catch_unwind(panic::AssertUnwindSafe(recover))
                .unwrap_or_else(|_| std::process::abort());

            // SAFETY: this value is empty due to the read above.
            unsafe { core::ptr::write(mut_ref, r) };
            panic::resume_unwind(err);
        },
        Ok((new_t, v)) => {
            // SAFETY: this value is empty due to the read above.
            unsafe { core::ptr::write(mut_ref, new_t) };
            v
        },
    }
}
