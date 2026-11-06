import {
  type BrowserGpuSceneSession,
  initializeBrowserGpuScene,
} from "../src/engine-client/wasm";
import { type Advance, checkedFields, FIELD_BYTES } from "./gpu-checkpoint";
import { measurePreviewPacing } from "./gpu-pacing";

export type { Advance } from "./gpu-checkpoint";

const INVENTORY_DRIFT_LIMIT = 1e-5;
type AdapterInfo = {
  vendor: string;
  architecture: string;
  description: string;
  isFallbackAdapter: boolean;
};
type ComputePass = {
  label: string;
  dispatchWorkgroups(x: number, y?: number, z?: number): void;
};

async function measure(scene: BrowserGpuSceneSession, ticks: number) {
  scene.reset();
  const painted = JSON.parse(scene.paint(240, 100, 4, "water")) as Advance;
  if (
    painted.tick !== 0 ||
    painted.acceptedTimeS !== 0 ||
    painted.stateRevision !== 1
  ) {
    throw new Error("GPU benchmark brush did not commit at zero model time");
  }
  if (!scene.render()) throw new Error("GPU benchmark cannot render the painted scene");
  const before = await checkedFields(await scene.checkpoint_prototype(), painted);
  const run = {
    completedTicks: 0,
    acceptedTimeS: 0,
    acceptedSubsteps: 0,
    attemptedCandidates: 0,
    refinementRetries: 0,
    readbackBytes: 0,
    rejectedReadbackBytes: 0,
    renderedFrames: 0,
    maxPressureIterations: 0,
    maxScaledResidual: 0,
    maxScaledDivergence: 0,
  };
  let last = painted;
  const started = performance.now();
  for (let calls = 0; run.completedTicks < ticks && calls < ticks * 64; calls += 1) {
    const next = JSON.parse(await scene.advance(8)) as Advance;
    if (
      next.epoch !== painted.epoch ||
      !Number.isFinite(next.acceptedTimeS) ||
      next.acceptedTimeS <= last.acceptedTimeS ||
      next.tick !== last.tick + Number(next.completedOuterTick) ||
      next.attemptedCandidates !== next.acceptedSubsteps + next.refinementRetries
    ) {
      throw new Error(
        "GPU benchmark advance has invalid accepted-time or attempt accounting",
      );
    }
    for (const key of [
      "acceptedSubsteps",
      "attemptedCandidates",
      "refinementRetries",
      "readbackBytes",
      "rejectedReadbackBytes",
    ] as const) {
      if (!Number.isSafeInteger(next[key]) || next[key] < 0) {
        throw new Error(`GPU benchmark has invalid ${key}`);
      }
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
      ) {
        throw new Error(
          "GPU benchmark completed tick exceeds reference pressure gates",
        );
      }
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
      throw new Error("GPU benchmark cannot render committed progress");
    run.renderedFrames += 1;
    run.completedTicks = next.tick;
    run.acceptedTimeS = next.acceptedTimeS;
    last = next;
  }
  // This bounded 16-byte probe waits for the last submitted render on the same queue.
  await scene.probe(240, 100);
  const wallSeconds = (performance.now() - started) / 1000;
  if (run.completedTicks !== ticks || Math.abs(run.acceptedTimeS - ticks / 60) > 1e-6) {
    throw new Error("GPU benchmark did not complete the requested model time");
  }
  const after = await checkedFields(await scene.checkpoint_prototype(), last);
  const relativeInventoryDrift = after.totals.map(
    (total, field) =>
      Math.abs(total - before.totals[field]) / Math.max(before.totals[field], 1e-12),
  );
  if (relativeInventoryDrift.some((drift) => drift > INVENTORY_DRIFT_LIMIT)) {
    throw new Error("GPU benchmark inventory drift exceeds its gate");
  }
  return {
    ...run,
    wallSeconds,
    modelSecondsPerWallSecond: run.acceptedTimeS / wallSeconds,
    finalQueueProbeReadbackBytes: 16,
    explicitCheckpointReadbackBytes: FIELD_BYTES * 2,
    before,
    after,
    relativeInventoryDrift,
  };
}

type BufferDevice = {
  createBuffer(descriptor: GPUBufferDescriptor): GPUBuffer;
};

async function benchmark(canvas: HTMLCanvasElement) {
  const gpu = navigator.gpu;
  if (!gpu) throw new Error("GPU benchmark requires WebGPU");
  const adapterRequests: { options: unknown; info: AdapterInfo | null }[] = [];
  const originalRequest = gpu.requestAdapter;
  gpu.requestAdapter = async (options) => {
    const adapter = await originalRequest.call(gpu, options);
    adapterRequests.push({
      options,
      info: adapter
        ? {
            vendor: adapter.info.vendor,
            architecture: adapter.info.architecture,
            description: adapter.info.description,
            isFallbackAdapter: adapter.info.isFallbackAdapter,
          }
        : null,
    });
    return adapter;
  };
  let scene: BrowserGpuSceneSession;
  const initializationStarted = performance.now();
  try {
    scene = await initializeBrowserGpuScene(canvas);
  } finally {
    gpu.requestAdapter = originalRequest;
  }
  const initializationWallMs = performance.now() - initializationStarted;
  try {
    const selectedAdapter = adapterRequests.at(-1)?.info;
    if (selectedAdapter?.isFallbackAdapter !== false) {
      throw new Error("GPU benchmark requires an identified hardware adapter");
    }
    const warmup = await measure(scene, 10);
    const runs = [];
    for (let run = 0; run < 3; run += 1) runs.push(await measure(scene, 60));
    const prototype = (
      globalThis as typeof globalThis & {
        GPUComputePassEncoder?: { prototype: ComputePass };
      }
    ).GPUComputePassEncoder?.prototype;
    if (!prototype) throw new Error("GPU benchmark dispatch tracing is unavailable");
    const devicePrototype = (
      globalThis as typeof globalThis & { GPUDevice?: { prototype: BufferDevice } }
    ).GPUDevice?.prototype;
    if (!devicePrototype)
      throw new Error("GPU benchmark buffer tracing is unavailable");
    const bufferAllocations: Record<string, { count: number; bytes: number }> = {};
    const originalBuffer = devicePrototype.createBuffer;
    devicePrototype.createBuffer = function (descriptor) {
      const label = descriptor.label ?? "unlabelled";
      const totals = bufferAllocations[label] ?? { count: 0, bytes: 0 };
      bufferAllocations[label] = totals;
      totals.count += 1;
      totals.bytes += Number(descriptor.size);
      return originalBuffer.call(this, descriptor);
    };
    const dispatches: Record<string, number> = {};
    const workgroups: Record<string, number> = {};
    const originalDispatch = prototype.dispatchWorkgroups;
    prototype.dispatchWorkgroups = function (x, y, z) {
      dispatches[this.label] = (dispatches[this.label] ?? 0) + 1;
      workgroups[this.label] = (workgroups[this.label] ?? 0) + x * (y ?? 1) * (z ?? 1);
      originalDispatch.call(this, x, y, z);
    };
    let tracedRun: Awaited<ReturnType<typeof measure>>;
    try {
      tracedRun = await measure(scene, 60);
    } finally {
      prototype.dispatchWorkgroups = originalDispatch;
      devicePrototype.createBuffer = originalBuffer;
    }
    const sustainedRun = await measure(scene, 300);
    const previewPacing = [];
    for (let run = 0; run < 3; run += 1) {
      const { last, ...pacing } = await measurePreviewPacing(scene, 60);
      const fields = await checkedFields(await scene.checkpoint_prototype(), last);
      previewPacing.push({ ...pacing, fields });
    }
    return {
      status: "pass",
      targetMet:
        runs.every((run) => run.modelSecondsPerWallSecond >= 1) &&
        sustainedRun.modelSecondsPerWallSecond >= 1,
      fixture: {
        width: 480,
        height: 270,
        brush: { x: 240, y: 100, radius: 4, material: "water" },
      },
      quality: {
        pressureIterations: 512,
        pressureBatchIterations: 16,
        pressureMethod: "component-aware Galerkin V-cycle",
        pressureAccumulation: "large and small f32 terms with explicit fma",
        pressureSmoothingSweeps: 2,
        pressureTolerance: 1e-5,
        closureRounds: 256,
        closurePairScheduling: "packed interior parity faces",
        transportParameters: "seven immutable buffers per encode",
        candidateStorage: "resident sequential scene candidate buffers",
      },
      measurement:
        "10-tick warm-up; fresh scene per run; render per advance; final queue probe included; checkpoints excluded",
      pressureObservationScope: "last accepted substep of each completed tick",
      adapter: JSON.parse(scene.adapter_info_json()) as unknown,
      adapterRequests,
      initializationWallMs,
      previewPacing,
      warmup,
      runs,
      sustainedRun,
      trace: {
        measurement: "separate instrumented run; duration is excluded from targetMet",
        dispatches,
        workgroups,
        bufferAllocations,
        allocationScope:
          "API createBuffer calls and requested bytes; cumulative, not peak resident memory",
        run: tracedRun,
      },
      userAgent: navigator.userAgent,
    };
  } finally {
    scene.dispose();
    scene.free();
  }
}

const result = document.getElementById("result");
const canvas = document.getElementById("scene");
if (!result || !(canvas instanceof HTMLCanvasElement))
  throw new Error("GPU benchmark DOM is missing");
try {
  result.textContent = JSON.stringify(await benchmark(canvas));
} catch (error) {
  result.textContent = JSON.stringify({ status: "fail", error: String(error) });
}
const token = new URL(location.href).searchParams.get("report-token");
if (token) {
  if (
    !/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
      token,
    )
  ) {
    throw new Error("Invalid GPU benchmark report token");
  }
  await fetch("http://127.0.0.1:4175/report", {
    method: "POST",
    body: JSON.stringify({ token, report: result.textContent }),
  });
}
