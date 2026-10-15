import { expect, test } from "bun:test";
import {
  boundedEvents,
  type ProfileTimeline,
  profileTrace,
} from "../../benchmarks/gpu-activity/timeline";

test("bounded diagnostics retain the beginning when async work completes in reverse order", () => {
  const journal = boundedEvents<{ started: number; completed: number }>(2);
  const first = journal.add({ started: 1, completed: 0 });
  const second = journal.add({ started: 2, completed: 0 });
  expect(journal.add({ started: 3, completed: 3 })).toBeNull();
  if (!first || !second) throw new Error("Missing reserved event slots");
  second.completed = 4;
  first.completed = 5;
  expect(journal.snapshot()).toEqual({
    capacity: 2,
    droppedEvents: 1,
    events: [
      { started: 1, completed: 5 },
      { started: 2, completed: 4 },
    ],
  });
});

test("trace export retains nanosecond precision without aligning GPU and host clocks", () => {
  const context = {
    advanceId: 3,
    epoch: 8,
    outerTick: 4,
    stateRevision: 9,
    acceptedTimeBeforeS: 0.05,
  };
  const timeline: ProfileTimeline = {
    version: 1,
    host: {
      clockDomain: "browser-performance-now",
      originMs: 42,
      capacity: 8,
      droppedEvents: 1,
      events: [
        {
          sequence: 1,
          type: "map",
          label: "pressure",
          startedMs: 44,
          completedMs: 46,
          outcome: "complete",
          context,
        },
      ],
    },
    gpu: {
      clockDomain: "gpu-timestamp",
      originTimestampNs: "9007199254740993000",
      capacity: 8,
      droppedEvents: 2,
      events: [
        {
          sequence: 1,
          label: "compute:pressure",
          beginTimestampNs: "9007199254740993500",
          endTimestampNs: "9007199254740994500",
          context,
        },
      ],
    },
    note: "Independent origins. No clock alignment was measured.",
  };
  const trace = profileTrace(timeline, {
    version: 1,
    clockDomain: "browser-performance-now",
    capacity: 8,
    droppedEvents: 3,
    events: [
      {
        sequence: 1,
        type: "pressureStatus",
        epoch: 8,
        outerTick: 4,
        candidateId: 1,
        attempt: 1,
        stateRevision: 9,
        acceptedTimeBeforeS: 0.05,
        encodedDtS: 1 / 60,
        phase: "pcgBatch",
        batchIndex: 0,
        requestedIterations: 32,
        mapStartedMs: 44,
        mapCompletedMs: 46,
        statusWords: [0, 0, 0, 0, 0, 0, 0, 0],
      },
    ],
  });
  const host = trace.traceEvents.find((event) => event.name === "map:pressure");
  const gpu = trace.traceEvents.find((event) => event.name === "compute:pressure");
  expect(host).toMatchObject({ pid: 1, ts: 2000, dur: 2000 });
  expect(gpu).toMatchObject({ pid: 2, ts: 0.5, dur: 1 });
  expect(
    trace.traceEvents.find((event) => event.name === "pressure:pcgBatch"),
  ).toMatchObject({
    pid: 1,
    ts: 2000,
    dur: 2000,
  });
  expect(trace.metadata).toMatchObject({
    hostDroppedEvents: 1,
    gpuDroppedEvents: 2,
    pressureDroppedEvents: 3,
    clockDomains: timeline.note,
  });
  expect(gpu?.args).toMatchObject(context);
});
