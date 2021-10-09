use std::num::NonZeroUsize;

pub fn map(duplicate: usize, channel: usize) -> usize {
    let s = duplicate + channel;

    // have to be careful not to overflow prematurely
    if s % 2 == 0 {
        (s / 2) * (s + 1) + channel
    } else {
        s * ((s + 1) / 2) + channel
    }
}

pub fn max_duplicates(channels: usize) -> usize {
    #[cfg(target_pointer_width = "16")]
    let channels_spacious = channels as u32;
    #[cfg(target_pointer_width = "32")]
    let channels_spacious = channels as u64;
    #[cfg(target_pointer_width = "64")]
    let channels_spacious = channels as u128;

    let sqrt = int_sqrt(2 * channels_spacious);
    let p = if sqrt % 2 == 0 {
        (sqrt / 2).checked_mul(sqrt + 1)
    } else {
        sqrt.checked_mul((sqrt + 1) / 2)
    };
    match p {
        Some(m) => {
            if m <= channels {
                sqrt
            } else {
                sqrt - 1
            }
        },
        None => sqrt - 1,
    }
}

pub fn max_channel(channels: NonZeroUsize, duplicate: usize) -> NonZeroUsize {
    let max_b = max_duplicates(channels.into()) - duplicate;

    let max_b = if max_b > last_b(channels.into()) {
        max_b - 1
    } else {
        max_b
    };

    unsafe { NonZeroUsize::new_unchecked(max_b) }
}

fn last_b(n: usize) -> usize {
    let a = max_duplicates(n);
    n - if a % 2 == 0 {
        (a / 2) * (a + 1)
    } else {
        a * ((a + 1) / 2)
    }
}

// this should really be u2*size. be careful.
fn int_sqrt(a: u128) -> usize {
    let mut x0 = a >> 1;

    if x0 != 0 {
        let mut x1 = (x0 + a / x0) >> 1;

        while x1 < x0 {
            x0 = x1;
            x1 = (x0 + a / x0) >> 1;
        }

        x0 as usize
    } else {
        a as usize
    }
}
