use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::Transposer;

pub trait StepMetadata<T: Transposer, S: StorageFamily> {
    type OriginalUnsaturated: Unpin;
    type RepeatUnsaturated: Unpin;
    type OriginalSaturating: Unpin;
    type RepeatSaturating: Unpin;
    type Saturated: Unpin;

    fn new_init() -> Self::OriginalSaturating;
    fn original_saturating(metadata: Self::OriginalUnsaturated) -> Self::OriginalSaturating;
    fn repeat_saturating(metadata: Self::RepeatUnsaturated) -> Self::RepeatSaturating;
    fn original_saturate(metadata: Self::OriginalSaturating, transposer: &T) -> Self::Saturated;
    fn repeat_saturate(metadata: Self::RepeatSaturating, transposer: &T) -> Self::Saturated;
    fn next_unsaturated(metadata: &Self::Saturated) -> Self::OriginalUnsaturated;
    fn desaturate_original_saturating(
        metadata: Self::OriginalSaturating,
    ) -> Self::OriginalUnsaturated;
    fn desaturate_repeat_saturating(metadata: Self::RepeatSaturating) -> Self::RepeatUnsaturated;
    fn desaturate_saturated(metadata: Self::Saturated, transposer: &T) -> Self::RepeatUnsaturated;
}

pub struct EmptyStepMetadata;

impl<T: Transposer, S: StorageFamily> StepMetadata<T, S> for EmptyStepMetadata {
    type OriginalUnsaturated = ();
    type RepeatUnsaturated = ();
    type OriginalSaturating = ();
    type RepeatSaturating = ();
    type Saturated = ();

    fn new_init() -> Self::OriginalSaturating {}
    fn original_saturating(_metadata: Self::OriginalUnsaturated) -> Self::OriginalSaturating {}
    fn repeat_saturating(_metadata: Self::RepeatUnsaturated) -> Self::RepeatSaturating {}
    fn original_saturate(_metadata: Self::OriginalSaturating, _transposer: &T) -> Self::Saturated {}
    fn repeat_saturate(_metadata: Self::RepeatSaturating, _transposer: &T) -> Self::Saturated {}
    fn next_unsaturated(_metadata: &Self::Saturated) -> Self::OriginalUnsaturated {}
    fn desaturate_original_saturating(
        _metadata: Self::OriginalSaturating,
    ) -> Self::OriginalUnsaturated {
    }
    fn desaturate_repeat_saturating(_metadata: Self::RepeatSaturating) -> Self::RepeatUnsaturated {}
    fn desaturate_saturated(
        _metadata: Self::Saturated,
        _transposer: &T,
    ) -> Self::RepeatUnsaturated {
    }
}
