import State, { PartialState } from './types/state';
import CozalEvent from './types/cozal-event'

// this is the logic of Cozal's real time interpretation of your context.
// this includes things like declarative rendering and hitsound playing.
export interface RepresentationContext<
    Rs extends State,
    S extends State,
> {
    logicState: S,
    setRepresentationState(partialState: PartialState<Rs>): void;
    addListener(listener: any): number;
    removeListener(id: number): void;
}

export interface RepresentationEventHandlerContext<
    Rs extends State,
    S extends State,
> extends RepresentationContext<Rs, S> {
    representationState: Rs,
}

// you can use this separately for different things, like video and audio for example
// or possibly to break out BGAs into their own representation.
export default function initializeRepresenter<
    Rs extends State,
    S extends State,
    E extends CozalEvent,
> (
    name: string,
    initFn: (context: RepresentationContext<Rs, S>) => (context: RepresentationEventHandlerContext<Rs, S>, event: E) => void,
): void {

}