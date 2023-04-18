/// A modified version of [`futures::task::Poll`], which has two new variants:
/// [`Scheduled`](self::SchedulePoll::Scheduled) and [`Done`](self::SchedulePoll::Done).
pub enum SourcePoll<T, E, S> {
    /// Indicates the poll is complete
    Ready {
        /// The requested state
        state:         S,
        /// The time of the next known event, if known.
        next_event_at: Option<T>,
    },

    /// Indicates information must be handled before state is emitted
    Interrupt {
        /// The time the information pertains to
        time: T,

        /// The type of interrupt
        interrupt: Interrupt<E>,
    },

    /// pending operation. caller will be woken up when progress can be made
    /// the channel this poll used must be retained.
    Pending,
}

/// The type of interrupt emitted from the source
pub enum Interrupt<E> {
    /// A new event is available.
    Event(E),

    /// An event followed by a finalize, for convenience.
    /// This should be identical to returning an event then a finalize for the same time.
    /// Useful for sources which never emit Rollbacks, so they can simply emit this interrupt
    /// for every event and nothing else.
    FinalizedEvent(E),

    /// All events before at or after time T must be discarded.
    Rollback,
    /// No event will ever be emitted before time T again.
    Finalize,
}

impl<T, E, S> SourcePoll<T, E, S>
where
    T: Ord + Copy,
{
    pub(crate) fn supress_state(self) -> SourcePoll<T, E, ()> {
        match self {
            Self::Ready {
                state: _,
                next_event_at,
            } => SourcePoll::Ready {
                state: (),
                next_event_at,
            },
            Self::Interrupt {
                time,
                interrupt: interrupt_type,
            } => SourcePoll::Interrupt {
                time,
                interrupt: interrupt_type,
            },
            Self::Pending => SourcePoll::Pending,
        }
    }
}

#[non_exhaustive]
pub enum SourcePollErr<T, Err> {
    OutOfBoundsChannel,
    PollAfterAdvance { advanced: T },
    PollBeforeDefault,
    SpecificError(Err),
}

pub type TrySourcePoll<T, E, S, Err> = Result<SourcePoll<T, E, S>, SourcePollErr<T, Err>>;
