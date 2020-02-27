import CozalEvent from "./types/cozal-event.ts";
import State, { PartialState } from "./types/state.ts";

interface CoreContext<S extends State, E extends CozalEvent> {
  setState(partialState: PartialState<S>): void;
  // added and removed events must sort later than the event currently being handled (no time traveling).
  addEvent(event: Omit<E, "id" | "source">): number; // returns event id
  removeEvent(eventID: number): void;
}

interface CoreEventHandlerContext<S extends State, E extends CozalEvent>
  extends CoreContext<S, E>
{
  state: S;
}

export default function initializeCore<S extends State, E extends CozalEvent>(
  name: string,
  initFn: (context: CoreContext<S, E>) => (
    context: CoreEventHandlerContext<S, E>,
    event: E
  ) => void
): void {
}
