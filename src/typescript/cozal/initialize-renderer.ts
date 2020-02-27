import State, { PartialState } from "./types/state.ts";
import CozalEvent from "./types/cozal-event.ts";

// this is the logic of Cozal's real time interpretation of your context.
// this includes things like declarative rendering and hitsound playing.
export interface RendererContext<
  Rs extends State,
  S extends State
> {
  logicState: S;
  setRendererState(partialState: PartialState<Rs>): void;
  addListener(listener: any): number;
  removeListener(id: number): void;
}

export interface RendererEventHandlerContext<
  Rs extends State,
  S extends State
>
  extends RendererContext<Rs, S>
{
  rendererState: Rs;
}

// you can use this separately for different things, like video and audio for example
// or possibly to break out BGAs into their own renderer.
export default function initializeRenderer<
  Rs extends State,
  S extends State,
  E extends CozalEvent
>(
  name: string,
  initFn: (context: RendererContext<Rs, S>) => (
    context: RendererEventHandlerContext<Rs, S>,
    event: E
  ) => void
): void {
}
