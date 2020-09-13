use std::cmp::Ordering;

// todo document.
pub struct FullOrd<T: PartialOrd>(pub T);

impl<T: PartialOrd> Ord for FullOrd<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        full_cmp(&self.0, &other.0)
    }
}

// impl<T: Ord> Ord for FullOrd<T> {
//     fn cmp(&self, other: &Self) -> Ordering {
//         self.0.cmp(other.0)
//     }
// }

impl<T: PartialOrd> PartialOrd for FullOrd<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: PartialOrd> Eq for FullOrd<T> {}

impl<T: PartialOrd> PartialEq for FullOrd<T> {
    fn eq(&self, other: &Self) -> bool {
        matches!(self.cmp(other), Ordering::Equal)
    }
}

pub fn full_cmp<T: PartialOrd>(this: &T, other: &T) -> Ordering {
    match this.partial_cmp(other) {
        Some(ord) => ord,
        None => Ordering::Equal,
    }
}
