use std::sync::Weak;
use std::task::Wake;

use transposer::schedule_storage::StorageFamily;
use transposer::step::StepMetadata;
use transposer::Transposer;
use util::stack_waker::StackWaker;
pub struct TransposeMetadata;

#[derive(Default)]
pub struct SaturatingMetadata {
    pub stack_waker:     Weak<StackWaker>,
    pub polling_channel: Option<usize>,
}

impl SaturatingMetadata {
    pub fn desaturate(self) {
        if let Some(w) = self.stack_waker.upgrade() {
            w.wake_by_ref();
        }
    }
}

impl<T: Transposer, S: StorageFamily> StepMetadata<T, S> for TransposeMetadata {
    type Unsaturated = ();

    type Saturating = SaturatingMetadata;

    type Saturated = ();

    fn new_unsaturated() -> Self::Unsaturated {}

    fn to_saturating(_metadata: Self::Unsaturated) -> Self::Saturating {
        Default::default()
    }

    fn to_saturated(metadata: Self::Saturating, _transposer: &T) -> Self::Saturated {
        metadata.desaturate();
    }

    fn desaturate_saturating(metadata: Self::Saturating) -> Self::Unsaturated {
        metadata.desaturate();
    }
}
