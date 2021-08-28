mod source;
// mod source_ext;
mod stateless_source;
mod timestamp;

pub use self::source::Source;
pub use self::source::SourceContext;
// pub use self::source_ext::SourceExt;
pub use self::stateless_source::StatelessSource;
pub use self::timestamp::Timestamp;

// pub mod gat_utilities {
//     pub struct Condition<const EXPRESSION: bool>;
//     pub trait True { }
//     impl True for Condition<true> { }
//     pub trait False { }
//     impl False for Condition<false> { }
// }
