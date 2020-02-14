import CozalEvent from './types/cozal-event.ts';
import State, { PartialState } from './types/state.ts';

interface LogicContext<S extends State, E extends CozalEvent> {
    setState(partialState: PartialState<S>): void;
    // added and removed events must sort later than the event currently being handled (no time traveling).
    addEvent(event: Omit<E, 'id' | 'source'>): number; // returns event id
    removeEvent(eventID: number): void;
}

interface LogicEventHandlerContext<S extends State, E extends CozalEvent> extends LogicContext<S, E> {
    state: S,
}

export default function initializeLogic<S extends State, E extends CozalEvent>(
    name: string,
    initFn: (context: LogicContext<S, E>) => (context: LogicEventHandlerContext<S, E>, event: E) => void,
): void {

}