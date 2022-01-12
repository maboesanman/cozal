mod interpolate_context;
mod interpolation;
mod lazy_state;
mod pointer_interpolation;
mod step;
mod step_metadata;
mod sub_step;

#[cfg(test)]
mod test;

pub use interpolation::Interpolation;
pub(crate) use pointer_interpolation::PointerInterpolation;
pub use step::*;
pub use step_metadata::StepMetadata;
