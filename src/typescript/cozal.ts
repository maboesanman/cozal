import PriorityQueue from "./utilities/immutable-priority-queue.ts";
import Stack from "./utilities/immutable-stack.ts";
import State from "./types/state";
import initializeLogic from "./initialize-logic.ts";
import initializeRepresenter from "./initialize-representer.ts";
import CozalEvent from "./types/cozal-event";
import CozalSystemEvent from "./types/cozal-system-event.ts";

interface CozalFrame {
    event: CozalSystemEvent,
    futureEvents: PriorityQueue<CozalEvent>,
    logicStates: { [K: string]: State },
    representationStates: { [K: string]: State },
}

const CozalInternal = {
    cozalHistory: new Stack<CozalFrame>().push({
        event: {
            source: 'system',
            time: 0,
            sort: 0,
            id: 0
        },
        futureEvents: new PriorityQueue("time", "sort", "id"),
        logicStates: {},
        representationStates: {},
    }),
    handleEvent(event: CozalEvent) {
        // rollback if necessary

        // call logic event handlers

        // call representation event handlers

        // push cozal frame
    },
};

const Cozal = {
    initializeLogic,
    initializeRepresenter,
    /*
    possibly some helpers here for reframing coordinates of components
    a "component" is simply another initFunction you call from the
    main initializeScreen function, possibly with modified arguments from context
    */
}

export default Cozal;

interface secondCounterState {
    count: number;
}

interface secondCounterEvent {
    source: 'secondCounter',
    id: number;
    time: number;
    sort?: number;
    message: string;
}

// an example logic 
Cozal.initializeLogic<
    secondCounterState,
    secondCounterEvent
>('secondCounter', (ctx) => {
    ctx.addEvent({
        time: 1000,
        sort: -1,
        message: 'hello',
    });
    ctx.setState({
        count: 0,
    })
    return (ctx, event) => {
        ctx.setState({
            count: (ctx.state as any).count + 1,
        });
        ctx.addEvent({
            time: event.time + 1000,
            sort: -1,
            message: 'hello',
        });
    };
});

// an example representer
Cozal.initializeRepresenter<
    null,
    secondCounterState,
    secondCounterEvent
>('secondMessenger', (ctx) => {
    return (ctx, event) => {
        (Deno as any).core.print(event.message)
    };
});
