use core::num::NonZeroUsize;
use std::collections::BTreeMap;

use super::OutChannelID;
use super::SrcChannelID;

pub struct AffinityMap {
    max_src_channels: NonZeroUsize,
    output_channels:  BTreeMap<OutChannelID, SrcChannelID>,
}

impl AffinityMap {
    pub fn new(max_channels: NonZeroUsize) -> Self {
        Self {
            max_src_channels: max_channels,
            output_channels:  BTreeMap::new(),
        }
    }

    pub fn get_affiliated_source_channel(&self, channel: OutChannelID) -> Option<SrcChannelID> {
        match self.output_channels.get(&channel) {
            Some(src_channel) => Some(*src_channel),
            None => {
                if channel < self.max_src_channels.into() {
                    Some(channel)
                } else {
                    None
                }
            },
        }
    }

    pub fn set_affiliation(&mut self, src_channel: SrcChannelID, out_channel: OutChannelID) {
        if src_channel == out_channel {
            self.output_channels.remove(&out_channel);
        } else {
            self.output_channels.insert(out_channel, src_channel);
        }
    }
}
