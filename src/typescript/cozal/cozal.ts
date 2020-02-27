import PriorityQueue from "./utilities/immutable-priority-queue.ts";
import Stack from "./utilities/immutable-stack.ts";
import State from "./types/state.ts";
import initializeCore from "./initialize-core.ts";
import initializeRenderer from "./initialize-renderer.ts";
import CozalEvent from "./types/cozal-event.ts";
import CozalSystemEvent from "./types/cozal-system-event.ts";
import ExpressionAPI from "./cozel-expression.ts";

interface CozalFrame {
  event: CozalSystemEvent;
  futureEvents: PriorityQueue<CozalEvent>;
  logicStates: { [K: string]: State; };
  representationStates: { [K: string]: State; };
}

const CozalInternal = {
  cozalHistory: new Stack<CozalFrame>().push({
    event: {
      source: "system",
      time: 0,
      sort: 0,
      id: 0
    },
    futureEvents: new PriorityQueue("time", "sort", "id"),
    logicStates: {},
    representationStates: {}
  }),
  handleEvent(event: CozalEvent) {
    // rollback if necessary

    // call logic event handlers

    // call representation event handlers

    // push cozal frame
  }
};

const Cozal = {
  initializeCore,
  initializeRenderer,
  Expression: ExpressionAPI
};

export default Cozal;

interface secondCounterState {
  count: number;
}

interface secondCounterEvent {
  source: "secondCounter";
  id: number;
  time: number;
  sort?: number;
  message: string;
}

// an example logic
Cozal.initializeCore<
  secondCounterState,
  secondCounterEvent
>("secondCounter", (ctx) => {
  ctx.addEvent({
    time: 1000,
    sort: -1,
    message: "hello"
  });
  ctx.setState({
    count: 0
  });
  return (ctx, event) => {
    ctx.setState({
      count: (ctx.state as any).count + 1
    });
    ctx.addEvent({
      time: event.time + 1000,
      sort: -1,
      message: "hello"
    });
  };
});

// an example representer
Cozal.initializeRenderer<
  null,
  secondCounterState,
  secondCounterEvent
>("secondMessenger", (ctx) => {
  return (ctx, event) => {
    (Deno as any).core.print(event.message);
  };
});
