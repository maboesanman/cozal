use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::Transposer;

pub trait StepMetadata<T: Transposer, S: StorageFamily> {
    type Unsaturated: Unpin;
    type Saturating: Unpin;
    type Saturated: Unpin;

    fn new_unsaturated() -> Self::Unsaturated;
    fn to_saturating(metadata: Self::Unsaturated) -> Self::Saturating;
    fn to_saturated(metadata: Self::Saturating, transposer: &T) -> Self::Saturated;

    fn desaturate_saturating(_metadata: Self::Saturating) -> Self::Unsaturated {
        Self::new_unsaturated()
    }
    fn desaturate_saturated(_metadata: Self::Saturated) -> Self::Unsaturated {
        Self::new_unsaturated()
    }
}

pub struct EmptyStepMetadata;

impl<T: Transposer, S: StorageFamily> StepMetadata<T, S> for EmptyStepMetadata {
    type Unsaturated = ();
    type Saturating = ();
    type Saturated = ();

    fn new_unsaturated() -> Self::Unsaturated {}
    fn to_saturating(_metadata: Self::Unsaturated) -> Self::Saturating {}
    fn to_saturated(_metadata: Self::Saturating, _transposer: &T) -> Self::Saturated {}
}
