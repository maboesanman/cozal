use std::collections::VecDeque;

pub fn get_with_next_mut<T>(v: &mut VecDeque<T>, i: usize) -> Option<(&mut T, Option<&mut T>)> {
    let (a, b) = v.as_mut_slices();

    let len_a = a.len();

    let (c, j) = match (i + 1).cmp(&len_a) {
        // both in first slice
        std::cmp::Ordering::Less => (a, i),

        // on the border
        std::cmp::Ordering::Equal => return Some((a.last_mut().unwrap(), b.first_mut())),

        // both in second slice
        std::cmp::Ordering::Greater => (b, i - len_a),
    };

    let (c1, c2) = c.split_at_mut(j + 1);
    Some((c1.last_mut().unwrap(), c2.first_mut()))
}
