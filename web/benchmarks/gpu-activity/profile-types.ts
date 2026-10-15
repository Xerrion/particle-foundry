import type { ActivityBuffer } from "./gpu-types";

export type ProfileBuffer = Omit<ActivityBuffer, "mapAsync"> & {
  label: string;
  mapAsync(mode: number, offset?: number, size?: number): Promise<void>;
};
export type QuerySet = { destroy(): void };
export type PassDescriptor = {
  label?: string;
  timestampWrites?: {
    querySet: QuerySet;
    beginningOfPassWriteIndex: number;
    endOfPassWriteIndex: number;
  };
};
export type ProfilePass = {
  label: string;
  setPipeline(pipeline: object): void;
  setBindGroup(
    index: number,
    group: object,
    offsets?: Iterable<number>,
    start?: number,
    length?: number,
  ): void;
  dispatchWorkgroups(x: number, y?: number, z?: number): void;
};
export type ProfileEncoder = {
  beginComputePass(descriptor?: PassDescriptor): ProfilePass;
  beginRenderPass(descriptor: PassDescriptor): object;
  resolveQuerySet(
    query: QuerySet,
    first: number,
    count: number,
    buffer: ProfileBuffer,
    offset: number,
  ): void;
  copyBufferToBuffer(
    source: ProfileBuffer,
    sourceOffset: number,
    target: ProfileBuffer,
    targetOffset: number,
    size: number,
  ): void;
  finish(descriptor?: object): object;
};
export type ProfileDevice = {
  createBuffer(descriptor: {
    label?: string;
    size: number;
    usage: number;
  }): ProfileBuffer;
  createBindGroup(descriptor: object): object;
  createQuerySet(descriptor: { type: "timestamp"; count: number }): QuerySet;
  createCommandEncoder(descriptor?: { label?: string }): ProfileEncoder;
  addEventListener(type: string, listener: EventListener): void;
  removeEventListener(type: string, listener: EventListener): void;
};
export type DeviceDescriptor = { requiredFeatures?: Iterable<string> };
export type ProfileAdapter = {
  features: Set<string>;
  info: {
    vendor: string;
    architecture: string;
    description: string;
    isFallbackAdapter: boolean;
  };
  requestDevice(descriptor?: DeviceDescriptor): Promise<ProfileDevice>;
};
export type ProfileGlobals = {
  GPUAdapter: { prototype: ProfileAdapter };
  GPUDevice: { prototype: ProfileDevice };
  GPUComputePassEncoder: { prototype: ProfilePass };
  GPUBuffer: { prototype: ProfileBuffer };
  GPUQueue: { prototype: { submit(commands: Iterable<object>): void } };
  GPUMapMode: { READ: number };
  GPUBufferUsage: {
    QUERY_RESOLVE: number;
    COPY_SRC: number;
    COPY_DST: number;
    MAP_READ: number;
  };
};
