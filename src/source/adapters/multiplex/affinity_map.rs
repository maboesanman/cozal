use std::{collections::BTreeMap, num::NonZeroUsize};

use super::{OutChannelID, SrcChannelID};


pub struct AffinityMap {
    max_src_channels: NonZeroUsize,
    source_channels: BTreeMap<SrcChannelID, OutChannelID>,
    output_channels: BTreeMap<OutChannelID, SrcChannelID>,
}

impl AffinityMap {
    pub fn new(max_channels: NonZeroUsize) -> Self {
        Self {
            max_src_channels: max_channels,
            source_channels: BTreeMap::new(),
            output_channels: BTreeMap::new(),
        }
    }

    pub fn get_affiliated_source_channel(&self, channel: OutChannelID) -> Option<SrcChannelID> {
        match self.output_channels.get(&channel) {
            Some(src_channel) => Some(*src_channel),
            None => if channel < self.max_src_channels.into() { Some(channel) } else { None },
        }
    }

    pub fn get_affiliated_output_channel(&self, channel: SrcChannelID) -> Option<OutChannelID> {
        match self.source_channels.get(&channel) {
            Some(out_channel) => Some(*out_channel),
            None => Some(channel),
        }
    }

    pub fn set_affiliation(&mut self, src_channel: SrcChannelID, out_channel: OutChannelID) {
        if src_channel == out_channel {
            self.source_channels.remove(&src_channel);
            self.output_channels.remove(&out_channel);
        } else {
            self.source_channels.insert(src_channel, out_channel);
            self.output_channels.insert(out_channel, src_channel);
        }
    }
}
