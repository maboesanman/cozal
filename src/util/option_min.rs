pub fn min_none_less<T: Ord>(a: Option<T>, b: Option<T>) -> Option<T> {
    match (a, b) {
        (Some(a), Some(b)) => match a.cmp(&b) {
            std::cmp::Ordering::Less => Some(a),
            std::cmp::Ordering::Equal => Some(a),
            std::cmp::Ordering::Greater => Some(b),
        },
        _ => None,
    }
}
