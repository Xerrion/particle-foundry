import { expect, test } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { captureBrowserTrace } from "../../scripts/browser-trace";

test("isolated CDP trace captures a bounded prefix and reads the returned stream once", async () => {
  const directory = await mkdtemp(join(tmpdir(), "particle-trace-test-"));
  const methods: string[] = [];
  const json = JSON.stringify({ traceEvents: [{ name: "gpu-test" }] });
  let traceConfig: unknown;
  const server = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    fetch(request, server) {
      if (server.upgrade(request)) return;
      return new Response("Expected WebSocket", { status: 400 });
    },
    websocket: {
      message(socket, data) {
        const message = JSON.parse(String(data));
        methods.push(message.method);
        let result = {};
        if (message.method === "Tracing.start")
          traceConfig = message.params.traceConfig;
        if (message.method === "Target.getTargets")
          result = { targetInfos: [{ type: "page", targetId: "isolated" }] };
        if (message.method === "Target.attachToTarget")
          result = { sessionId: "page-session" };
        if (message.method === "IO.read")
          result = {
            data: Buffer.from(json).toString("base64"),
            base64Encoded: true,
            eof: true,
          };
        socket.send(JSON.stringify({ id: message.id, result }));
        if (message.method === "Tracing.end") {
          socket.send(
            JSON.stringify({
              method: "Tracing.tracingComplete",
              params: { stream: "trace", dataLossOccurred: true },
            }),
          );
        }
      },
    },
  });
  let trace: Awaited<ReturnType<typeof captureBrowserTrace>> | undefined;
  try {
    await Bun.write(
      join(directory, "DevToolsActivePort"),
      `${server.port}\n/devtools/browser/isolated\n`,
    );
    const output = join(directory, "trace.json");
    trace = await captureBrowserTrace(
      directory,
      () => null,
      new URL("http://127.0.0.1:1234/fixture"),
      output,
      1,
    );
    const stopped = await trace.stop();
    expect(stopped).toMatchObject({
      bytes: Buffer.byteLength(json),
      dataLossOccurred: true,
      stoppedBy: "benchmark-complete",
    });
    expect(await Bun.file(output).json()).toEqual(JSON.parse(json));
    expect(await trace.stop()).toBe(stopped);
    expect(methods.filter((method) => method === "Tracing.end")).toHaveLength(1);
    expect(traceConfig).toMatchObject({
      recordMode: "recordUntilFull",
      traceBufferSizeInKb: 32768,
    });
  } finally {
    trace?.close();
    await server.stop(true);
    await rm(directory, { recursive: true, force: true });
  }
});
