use std::mem::MaybeUninit;

// SAFETY: this runs drop and leaves the data as uninit. it is only to be used if the normal drop call has been supressed.
pub unsafe fn drop_mut_leave_uninit<T>(mut_ref: &mut T) {
    unsafe {
        let maybe: &mut MaybeUninit<T> = core::mem::transmute(mut_ref);
        maybe.assume_init_drop();
    }
}

// SAFETY: this takes the data leaves the ref as uninit. it is only to be used if the normal drop call has been supressed.
pub unsafe fn take_mut_leave_uninit<T>(mut_ref: &mut T) -> T {
    unsafe {
        let maybe: &mut MaybeUninit<T> = core::mem::transmute(mut_ref);
        maybe.assume_init_read()
    }
}
