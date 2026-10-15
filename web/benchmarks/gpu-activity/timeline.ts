export type AdvanceContext = {
  advanceId: number;
  epoch: number;
  outerTick: number;
  stateRevision: number;
  acceptedTimeBeforeS: number;
};
export type HostEvent = {
  sequence: number;
  type: "submit" | "map" | "advance";
  label: string;
  startedMs: number;
  completedMs: number;
  outcome: "complete" | "error";
  context: AdvanceContext | null;
};
export type GpuEvent = {
  sequence: number;
  label: string;
  beginTimestampNs: string;
  endTimestampNs: string;
  context: AdvanceContext | null;
};
export type ProfileTimeline = {
  version: 1;
  host: {
    clockDomain: "browser-performance-now";
    originMs: number;
    capacity: number;
    droppedEvents: number;
    events: HostEvent[];
  };
  gpu: {
    clockDomain: "gpu-timestamp";
    originTimestampNs: string | null;
    capacity: number;
    droppedEvents: number;
    events: GpuEvent[];
  };
  note: string;
};

/** Preserve the diagnostic prefix. The returned slot keeps async completion order from changing it. */
export function boundedEvents<T>(capacity: number) {
  if (!Number.isSafeInteger(capacity) || capacity < 1 || capacity > 65_536)
    throw new Error("Timeline capacity must be between 1 and 65536 events");
  const events: T[] = [];
  let droppedEvents = 0;
  return {
    add(event: T) {
      if (events.length === capacity) {
        droppedEvents += 1;
        return null;
      }
      events.push(event);
      return event;
    },
    snapshot: () => ({ capacity, droppedEvents, events }),
  };
}

export type PressureContext = {
  epoch: number;
  outerTick: number;
  candidateId: number;
  attempt: number;
  stateRevision: number;
  acceptedTimeBeforeS: number;
  encodedDtS: number;
  sequence: number;
};
export type PressureEvent = PressureContext &
  (
    | { type: "candidateBegin"; timestampMs: number }
    | {
        type: "pressureStatus";
        phase: "initialization" | "pcgBatch" | "finalGate";
        batchIndex: number;
        requestedIterations: number;
        mapStartedMs: number;
        mapCompletedMs: number;
        statusWords: number[];
      }
    | {
        type: "candidateOutcome";
        timestampMs: number;
        outcome: "accepted" | "numericalRejection" | "terminalRejection";
      }
  );
export type PressureJournal = {
  version: 1;
  clockDomain: "browser-performance-now" | "native-instant";
  capacity: number;
  droppedEvents: number;
  events: PressureEvent[];
};

type TraceEvent = {
  name: string;
  ph: "X" | "i" | "M";
  pid: number;
  tid: number;
  ts?: number;
  dur?: number;
  s?: "t";
  args: object;
};

/** GPU and host tracks use independent zero points. Their horizontal positions do not establish a queue delay. */
export function profileTrace(timeline: ProfileTimeline, pressure?: PressureJournal) {
  const traceEvents: TraceEvent[] = [
    {
      name: "process_name",
      ph: "M",
      pid: 1,
      tid: 0,
      args: { name: "Host performance.now" },
    },
    {
      name: "process_name",
      ph: "M",
      pid: 2,
      tid: 0,
      args: { name: "GPU timestamps (independent origin)" },
    },
  ];
  for (const event of timeline.host.events) {
    traceEvents.push({
      name: `${event.type}:${event.label}`,
      ph: "X",
      pid: 1,
      tid: event.type === "advance" ? 1 : event.type === "submit" ? 2 : 3,
      ts: (event.startedMs - timeline.host.originMs) * 1000,
      dur: (event.completedMs - event.startedMs) * 1000,
      args: { sequence: event.sequence, outcome: event.outcome, ...event.context },
    });
  }
  if (timeline.gpu.originTimestampNs !== null) {
    const origin = BigInt(timeline.gpu.originTimestampNs);
    for (const event of timeline.gpu.events) {
      const begin = BigInt(event.beginTimestampNs);
      const end = BigInt(event.endTimestampNs);
      traceEvents.push({
        name: event.label,
        ph: "X",
        pid: 2,
        tid: 1,
        ts: Number(begin - origin) / 1000,
        dur: Number(end - begin) / 1000,
        args: { sequence: event.sequence, ...event.context },
      });
    }
  }
  if (pressure?.clockDomain === "browser-performance-now") {
    for (const event of pressure.events) {
      const status = event.type === "pressureStatus";
      const startedMs = status ? event.mapStartedMs : event.timestampMs;
      traceEvents.push({
        name: status ? `pressure:${event.phase}` : event.type,
        ph: status ? "X" : "i",
        pid: 1,
        tid: 4,
        ts: (startedMs - timeline.host.originMs) * 1000,
        ...(status
          ? { dur: (event.mapCompletedMs - startedMs) * 1000 }
          : { s: "t" as const }),
        args: event,
      });
    }
  }
  return {
    traceEvents,
    displayTimeUnit: "ms",
    metadata: {
      clockDomains: timeline.note,
      hostDroppedEvents: timeline.host.droppedEvents,
      gpuDroppedEvents: timeline.gpu.droppedEvents,
      pressureDroppedEvents: pressure?.droppedEvents ?? null,
      pressureClockDomain: pressure?.clockDomain ?? null,
    },
  };
}
