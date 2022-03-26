use std::sync::Weak;

use super::storage::TransposeStorage;
use crate::transposer::schedule_storage::StorageFamily;
use crate::transposer::step::{Metadata, MetadataMut, StepMetadata};
use crate::transposer::Transposer;
use crate::util::stack_waker::StackWaker;

pub struct TransposeStepMetadata;

pub struct RepeatSaturatingMetadata {
    pub stack_waker: Weak<StackWaker>,
}

pub fn unwrap_repeat_saturating<'a, T: Transposer>(
    metadata: Metadata<'a, T, TransposeStorage, TransposeStepMetadata>,
) -> &'a RepeatSaturatingMetadata {
    match metadata {
        Metadata::RepeatSaturating(r) => r,
        _ => unreachable!(),
    }
}

pub fn unwrap_repeat_saturating_mut<'a, T: Transposer>(
    metadata: MetadataMut<'a, T, TransposeStorage, TransposeStepMetadata>,
) -> &'a mut RepeatSaturatingMetadata {
    match metadata {
        MetadataMut::RepeatSaturating(r) => r,
        _ => unreachable!(),
    }
}

impl<T: Transposer, S: StorageFamily> StepMetadata<T, S> for TransposeStepMetadata {
    type OriginalUnsaturated = ();
    type RepeatUnsaturated = ();
    type OriginalSaturating = ();
    type RepeatSaturating = RepeatSaturatingMetadata;
    type Saturated = ();

    fn new_init() -> Self::OriginalSaturating {}
    fn original_saturating(_metadata: Self::OriginalUnsaturated) -> Self::OriginalSaturating {}
    fn repeat_saturating(_metadata: Self::RepeatUnsaturated) -> Self::RepeatSaturating {
        RepeatSaturatingMetadata {
            stack_waker: StackWaker::new_empty(),
        }
    }
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
