use self::offload_source::OffloadSource;
use crate::Source;

mod offload_future;
mod offload_source;

pub fn offload<Src: Source>(source: Src) -> (OffloadSource<Src>, OffloadSource<Src>) {
    unimplemented!()
}
