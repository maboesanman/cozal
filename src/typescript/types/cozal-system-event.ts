import CozalEvent from "./cozal-event.ts";

interface CozalSystemEvent extends CozalEvent {
    source: 'system';
    id: number;
    time: number;
    sort: 0;
}

export default CozalSystemEvent;