// These interfaces cover only the browser WebGPU operations used by this diagnostic.
// The observer does not depend on unrelated WebGPU methods or mutable descriptor fields.
export type ActivityBuffer = {
  mapAsync(mode: number): Promise<void>;
  getMappedRange(): ArrayBuffer;
  unmap(): void;
  destroy(): void;
};
export type ActivityShader = {
  getCompilationInfo(): Promise<{ messages: { type: string; message: string }[] }>;
};
export type ActivityPipeline = { getBindGroupLayout(index: number): object };
export type ShaderDescriptor = { label?: string; code: string };
export type PipelineDescriptor = {
  label?: string;
  layout: "auto" | object;
  compute: { module: ActivityShader; entryPoint?: string };
};
export type ActivityEncoder = {
  copyBufferToBuffer(
    source: ActivityBuffer,
    sourceOffset: number,
    destination: ActivityBuffer,
    destinationOffset: number,
    size: number,
  ): void;
  finish(): object;
};
export type ActivityDevice = {
  createShaderModule(descriptor: ShaderDescriptor): ActivityShader;
  createComputePipeline(descriptor: PipelineDescriptor): ActivityPipeline;
  createComputePipelineAsync(descriptor: PipelineDescriptor): Promise<ActivityPipeline>;
  createBuffer(descriptor: {
    label: string;
    size: number;
    usage: number;
  }): ActivityBuffer;
  createBindGroup(descriptor: {
    label: string;
    layout: object;
    entries: { binding: number; resource: { buffer: ActivityBuffer } }[];
  }): object;
  createCommandEncoder(descriptor: { label: string }): ActivityEncoder;
  queue: {
    writeBuffer(buffer: ActivityBuffer, offset: number, data: Uint32Array): void;
    submit(commands: object[]): void;
    onSubmittedWorkDone(): Promise<void>;
  };
  addEventListener(type: string, listener: EventListener): void;
  removeEventListener(type: string, listener: EventListener): void;
};
export type ActivityPass = {
  label: string;
  setPipeline(pipeline: ActivityPipeline): void;
  setBindGroup(index: number, bindGroup: object): void;
};
