import type { BrowserGpuSceneSession } from "../../src/engine-client/wasm";
import {
  type Advance,
  type CommittedStamp,
  checkedFields,
  FIELD_BYTES,
} from "../gpu-checkpoint";
import { type FixtureName, prepareFixture } from "./fixtures";
import type { AdvanceContext } from "./timeline";

const animationFrame = () =>
  new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
const percentile = (values: number[], fraction: number) =>
  values[Math.min(values.length - 1, Math.ceil(values.length * fraction) - 1)] ?? 0;

export type RunOptions = {
  fixture: FixtureName;
  ticks: number;
  pacing: boolean;
  beforeAdvance?: (last: CommittedStamp, context: AdvanceContext) => void;
  afterAdvance?: (context: AdvanceContext, outcome: "complete" | "error") => void;
  beforeTimer?: () => void;
  afterTimer?: () => void;
  saveCheckpoint?: (phase: "before" | "after", bytes: Uint8Array) => Promise<void>;
};

export async function measureActivityFixture(
  scene: BrowserGpuSceneSession,
  options: RunOptions,
) {
  const resetStamp = scene.reset();
  await scene.probe(240, 100);
  const paintStarted = performance.now();
  let last = prepareFixture({ ...scene, reset: () => resetStamp }, options.fixture);
  const paintEnqueueMs = performance.now() - paintStarted;
  if (!scene.render())
    throw new Error("Activity fixture cannot render its initial state");
  await scene.probe(240, 100);
  const paintQueueCompletionMs = performance.now() - paintStarted;
  const beforeBytes = await scene.checkpoint_prototype();
  const before = await checkedFields(beforeBytes, last);
  await options.saveCheckpoint?.("before", beforeBytes);
  const initialStamp = last;
  const run = {
    acceptedSubsteps: 0,
    attemptedCandidates: 0,
    refinementRetries: 0,
    readbackBytes: 0,
    rejectedReadbackBytes: 0,
    maxPressureIterations: 0,
    maxScaledResidual: 0,
    maxScaledDivergence: 0,
  };
  const renderTimes: number[] = [];
  options.beforeTimer?.();
  const started = performance.now();
  for (
    let calls = 0;
    last.tick < options.ticks && calls < options.ticks * 64;
    calls += 1
  ) {
    if (options.pacing) await animationFrame();
    const context: AdvanceContext | undefined =
      options.beforeAdvance || options.afterAdvance
        ? {
            advanceId: calls + 1,
            epoch: last.epoch,
            outerTick: last.tick + 1,
            stateRevision: last.stateRevision,
            acceptedTimeBeforeS: last.acceptedTimeS,
          }
        : undefined;
    if (context) options.beforeAdvance?.(last, context);
    let next: Advance;
    try {
      next = JSON.parse(await scene.advance(8)) as Advance;
    } catch (error) {
      if (context) options.afterAdvance?.(context, "error");
      throw error;
    }
    if (context) options.afterAdvance?.(context, "complete");
    if (
      next.epoch !== initialStamp.epoch ||
      !Number.isFinite(next.acceptedTimeS) ||
      next.acceptedTimeS <= last.acceptedTimeS ||
      next.tick !== last.tick + Number(next.completedOuterTick) ||
      next.attemptedCandidates !== next.acceptedSubsteps + next.refinementRetries
    )
      throw new Error(
        "Activity fixture has invalid accepted-time or attempt accounting",
      );
    for (const key of [
      "acceptedSubsteps",
      "attemptedCandidates",
      "refinementRetries",
      "readbackBytes",
      "rejectedReadbackBytes",
    ] as const) {
      if (!Number.isSafeInteger(next[key]) || next[key] < 0)
        throw new Error(`Invalid ${key}`);
      run[key] += next[key];
    }
    if (next.completedOuterTick) {
      if (
        next.pressureIterations === null ||
        next.scaledResidual === null ||
        next.scaledDivergence === null ||
        !Number.isSafeInteger(next.pressureIterations) ||
        next.pressureIterations < 0 ||
        next.pressureIterations > 512 ||
        ![next.scaledResidual, next.scaledDivergence].every(
          (value) => Number.isFinite(value) && value >= 0 && value <= 1e-5,
        )
      )
        throw new Error("Activity fixture exceeds the reference pressure gates");
      run.maxPressureIterations = Math.max(
        run.maxPressureIterations,
        next.pressureIterations,
      );
      run.maxScaledResidual = Math.max(run.maxScaledResidual, next.scaledResidual);
      run.maxScaledDivergence = Math.max(
        run.maxScaledDivergence,
        next.scaledDivergence,
      );
    }
    if (!scene.render())
      throw new Error("Activity fixture cannot render committed progress");
    renderTimes.push(performance.now());
    last = next;
  }
  await scene.probe(240, 100);
  const wallSeconds = (performance.now() - started) / 1000;
  options.afterTimer?.();
  if (
    last.tick !== options.ticks ||
    Math.abs(last.acceptedTimeS - options.ticks / 60) > 1e-6
  ) {
    throw new Error("Activity fixture did not complete the requested accepted time");
  }
  const afterBytes = await scene.checkpoint_prototype();
  const after = await checkedFields(afterBytes, last);
  await options.saveCheckpoint?.("after", afterBytes);
  const relativeInventoryDrift = after.totals.map(
    (total, field) =>
      Math.abs(total - before.totals[field]) / Math.max(before.totals[field], 1e-12),
  );
  if (relativeInventoryDrift.some((drift) => drift > 1e-5)) {
    throw new Error("Activity fixture inventory drift exceeds the reference gate");
  }
  const intervals = renderTimes
    .slice(1)
    .map((time, index) => time - renderTimes[index]);
  intervals.sort((a, b) => a - b);
  return {
    ...run,
    fixture: options.fixture,
    paintEnqueueMs,
    paintQueueCompletionMs,
    paintMeasurementScope:
      "Reset completion is excluded. Queue completion includes paint, one render and a 16-byte probe. This does not measure input-to-photon latency.",
    policy: options.pacing
      ? "requestAnimationFrame, advance(8), render"
      : "advance(8), render",
    measurementScope:
      "Render submissions are presentation proxies. Displayed FPS and input latency remain unqualified.",
    completedTicks: last.tick,
    acceptedTimeS: last.acceptedTimeS,
    wallSeconds,
    modelSecondsPerWallSecond: last.acceptedTimeS / wallSeconds,
    renderSubmissions: renderTimes.length,
    renderSubmissionsPerSecond: renderTimes.length / wallSeconds,
    frameIntervalP50Ms: percentile(intervals, 0.5),
    frameIntervalP95Ms: percentile(intervals, 0.95),
    frameIntervalMaxMs: intervals.at(-1) ?? 0,
    intervalsOver60FpsBudget: intervals.filter((ms) => ms > 1000 / 60).length,
    initialStamp,
    finalStamp: last,
    before,
    after,
    relativeInventoryDrift,
    fieldsChanged: before.fieldSha256 !== after.fieldSha256,
    finalQueueProbeReadbackBytes: 16,
    explicitCheckpointReadbackBytes: 2 * FIELD_BYTES,
  };
}
