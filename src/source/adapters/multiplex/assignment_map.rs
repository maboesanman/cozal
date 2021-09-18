use core::num::NonZeroUsize;
use std::collections::BTreeMap;

use super::{OutChannelID, SrcChannelID};

pub struct AssignmentMap<Time: Ord + Copy> {
    max_src_channels: NonZeroUsize,
    source_channels:  BTreeMap<SrcChannelID, OutChannelID>,
    output_channels:  BTreeMap<OutChannelID, Assignment<Time>>,
}

#[derive(Clone, Copy)]
pub struct Assignment<Time: Ord + Copy> {
    pub poll_type:      PollType,
    pub time:           Time,
    pub source_channel: SrcChannelID,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PollType {
    Poll,
    PollForget,
    PollEvents,
}

impl<Time: Ord + Copy> AssignmentMap<Time> {
    pub fn new(max_channels: NonZeroUsize) -> Self {
        Self {
            max_src_channels: max_channels,
            source_channels:  BTreeMap::new(),
            output_channels:  BTreeMap::new(),
        }
    }

    pub fn get_unassigned_source_channel(&self) -> Option<SrcChannelID> {
        let mut channel: SrcChannelID = 0;
        loop {
            if channel >= self.max_src_channels.into() {
                break None
            }

            if self.get_assigned_output_channel(channel).is_some() {
                channel += 1;
                continue
            } else {
                break Some(channel)
            }
        }
    }

    pub fn get_assigned_source_channel(&self, channel: OutChannelID) -> Option<Assignment<Time>> {
        self.output_channels.get(&channel).copied()
    }

    pub fn get_assigned_output_channel(&self, channel: SrcChannelID) -> Option<OutChannelID> {
        match self.source_channels.get(&channel) {
            Some(out_channel) => Some(*out_channel),
            None => Some(channel),
        }
    }

    pub fn assign(&mut self, out_channel: OutChannelID, assignment: Assignment<Time>) {
        self.source_channels
            .insert(assignment.source_channel, out_channel);
        self.output_channels.insert(out_channel, assignment);
    }

    pub fn unassign(&mut self, src_channel: SrcChannelID) {
        if let Some(out_channel) = self.source_channels.remove(&src_channel) {
            self.output_channels.remove(&out_channel);
        };
    }
}
