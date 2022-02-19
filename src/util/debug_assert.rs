
#[cfg(not(debug_assertions))]
use std::hint::unreachable_unchecked;

#[inline(always)]
#[rustfmt::skip]
pub unsafe fn debug_unreachable() -> ! {
    #[cfg(debug_assertions)]
    unreachable!();

    #[cfg(not(debug_assertions))]
    unsafe {
        unreachable_unchecked()
    };
}

#[inline(always)]
#[rustfmt::skip]
pub unsafe fn debug_unwrap<T>(option: Option<T>) -> T {
    #[cfg(debug_assertions)]
    return option.unwrap();

    #[cfg(not(debug_assertions))]
    unsafe {
        option.unwrap_unchecked()
    }
}
