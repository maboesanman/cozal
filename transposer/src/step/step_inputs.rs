use std::any::TypeId;
use std::collections::BTreeMap;
use std::pin::Pin;

use futures_core::Future;
use type_erased_vec::TypeErasedVec;

use crate::context::HandleInputContext;
use crate::{Transposer, TransposerInput, TransposerInputEventHandler};

pub struct StepInputs<T: Transposer> {
    pub time: T::Time,

    // these btreesets are all of different values. they are transmuted before use.
    inputs: BTreeMap<u64, StepInputsEntry<T>>,
}

type HandlerFunction<T> = for<'a> fn(
    time: <T as Transposer>::Time,
    &'a mut T,
    &'a mut dyn HandleInputContext<'_, T>,
    &'a TypeErasedVec,
) -> Pin<Box<dyn 'a + Future<Output = ()>>>;

struct StepInputsEntry<T: Transposer> {
    // keep this sorted
    values:        TypeErasedVec,
    input_type_id: TypeId,
    handler:       HandlerFunction<T>,
}

impl<T: Transposer> StepInputsEntry<T> {
    fn new<I: TransposerInput<Base = T>>() -> Self
    where
        T: TransposerInputEventHandler<I>,
    {
        Self {
            values:        TypeErasedVec::new::<I::InputEvent>(),
            input_type_id: TypeId::of::<I>(),
            handler:       |time, t, cx, set| {
                // SAFETY: this came from the assignment to values, which erased the I::InputEvent type
                let set = unsafe { set.get::<I::InputEvent>() };
                Box::pin(async move {
                    for i in set.iter() {
                        T::handle_input(t, i, cx).await
                    }
                })
            },
        }
    }

    fn add_input<I: TransposerInput<Base = T>>(&mut self, time: T::Time, input: I::InputEvent)
    where
        T: TransposerInputEventHandler<I>,
    {
        if TypeId::of::<I>() != self.input_type_id {
            panic!()
        }

        // SAFETY: this matches the type because I has a TypeId that matches the one that created it.
        let mut set = unsafe { self.values.get_mut() };

        let i =
            set.partition_point(|existing| T::sort_input_events(time, &input, existing).is_lt());

        set.insert(i, input);
    }
}

impl<T: Transposer> StepInputs<T> {
    pub async fn handle(&self, transposer: &mut T, cx: &mut dyn HandleInputContext<'_, T>) {
        for (_, i) in self.inputs.iter() {
            (i.handler)(self.time, transposer, cx, &i.values).await;
        }
    }

    pub fn add_event<I: TransposerInput<Base = T>>(&mut self, event: I::InputEvent)
    where
        T: TransposerInputEventHandler<I>,
    {
        let step_inputs_entry = match self.inputs.entry(I::SORT) {
            std::collections::btree_map::Entry::Vacant(v) => v.insert(StepInputsEntry::new()),
            std::collections::btree_map::Entry::Occupied(o) => o.into_mut(),
        };

        step_inputs_entry.add_input(self.time, event);
    }

    pub fn time(&self) -> T::Time {
        self.time
    }
}
