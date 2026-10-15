import { readFile, rename, rm } from "node:fs/promises";
import { join } from "node:path";

export const browserTraceCategories = [
	"devtools.timeline",
	"disabled-by-default-devtools.timeline",
	"disabled-by-default-devtools.timeline.frame",
	"blink.user_timing",
	"v8",
	"gpu",
	"dawn",
];
export const browserTraceBufferKb = 32 * 1024;
const maxTraceBytes = 128 * 1024 * 1024;

type ProtocolMessage = {
	id?: number;
	method?: string;
	params?: Record<string, unknown>;
	result?: Record<string, unknown>;
	error?: { message: string };
};

/** Use the debugging endpoint written by this isolated browser. Never discover an existing user's browser. */
export async function captureBrowserTrace(
	profileDirectory: string,
	exitCode: () => number | null,
	url: URL,
	outputPath: string,
	maximumSeconds: number,
) {
	const deadline = Date.now() + 15_000;
	let endpoint: URL | undefined;
	while (Date.now() < deadline) {
		if (exitCode() !== null) throw new Error("Trace browser exited during startup");
		try {
			const [port, path] = (await readFile(join(profileDirectory, "DevToolsActivePort"), "utf8"))
				.trim()
				.split("\n");
			if (!/^\d+$/.test(port) || !path?.startsWith("/devtools/browser/"))
				throw new Error("Invalid isolated browser debugging endpoint");
			endpoint = new URL(`ws://127.0.0.1:${port}${path}`);
			break;
		} catch (error) {
			if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
			await Bun.sleep(50);
		}
	}
	if (!endpoint) throw new Error("Isolated browser did not expose a debugging endpoint");
	const socket = new WebSocket(endpoint);
	let nextId = 0;
	const pending = new Map<
		number,
		{ resolve: (result: Record<string, unknown>) => void; reject: (error: Error) => void }
	>();
	let traceComplete!: (message: Record<string, unknown>) => void;
	let traceFailed!: (error: Error) => void;
	const finished = new Promise<Record<string, unknown>>((resolve, reject) => {
		traceComplete = resolve;
		traceFailed = reject;
	});
	// A disconnect can arrive before stop() awaits the completion event.
	void finished.catch(() => undefined);
	socket.addEventListener("message", (event) => {
		const message = JSON.parse(String(event.data)) as ProtocolMessage;
		if (message.id !== undefined) {
			const operation = pending.get(message.id);
			pending.delete(message.id);
			if (message.error) operation?.reject(new Error(message.error.message));
			else operation?.resolve(message.result ?? {});
		} else if (message.method === "Tracing.tracingComplete") {
			traceComplete(message.params ?? {});
		}
	});
	socket.addEventListener("close", () => {
		const error = new Error("Isolated browser trace connection closed");
		for (const operation of pending.values()) operation.reject(error);
		pending.clear();
		traceFailed(error);
	});
	function command(method: string, params = {}, sessionId?: string) {
		const id = ++nextId;
		return new Promise<Record<string, unknown>>((resolve, reject) => {
			const timer = setTimeout(() => {
				pending.delete(id);
				reject(new Error(`Browser trace command timed out: ${method}`));
			}, 15_000);
			pending.set(id, {
				resolve: (value) => {
					clearTimeout(timer);
					resolve(value);
				},
				reject: (error) => {
					clearTimeout(timer);
					reject(error);
				},
			});
			socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
		});
	}
	await new Promise<void>((resolve, reject) => {
		const timer = setTimeout(() => reject(new Error("Browser trace socket timed out")), 15_000);
		socket.addEventListener(
			"open",
			() => {
				clearTimeout(timer);
				resolve();
			},
			{ once: true },
		);
		socket.addEventListener(
			"error",
			() => {
				clearTimeout(timer);
				reject(new Error("Browser trace socket failed"));
			},
			{ once: true },
		);
	});
	let timer: ReturnType<typeof setTimeout> | undefined;
	let stopPromise:
		| Promise<{ bytes: number; dataLossOccurred: boolean; stoppedBy: string }>
		| undefined;
	let startedAt = performance.now();
	async function save(stoppedBy: string) {
		clearTimeout(timer);
		const capturedSeconds = (performance.now() - startedAt) / 1000;
		await command("Tracing.end");
		const completionTimeout = setTimeout(
			() => traceFailed(new Error("Browser trace flush timed out")),
			30_000,
		);
		let handle: string;
		try {
			const result = await finished;
			if (typeof result.stream !== "string")
				throw new Error("Browser trace did not return a stream");
			handle = result.stream;
		} finally {
			clearTimeout(completionTimeout);
		}
		const partial = `${outputPath}.partial`;
		const writer = Bun.file(partial).writer();
		let bytes = 0;
		let ended = false;
		try {
			for (;;) {
				const chunk = await command("IO.read", { handle, size: 1024 * 1024 });
				if (typeof chunk.data !== "string" || typeof chunk.eof !== "boolean")
					throw new Error("Invalid browser trace stream chunk");
				const data = Buffer.from(chunk.data, chunk.base64Encoded ? "base64" : "utf8");
				bytes += data.length;
				if (bytes > maxTraceBytes) throw new Error("Browser trace exceeds 128 MiB storage limit");
				writer.write(data);
				if (chunk.eof) break;
			}
			await writer.end();
			ended = true;
			await rename(partial, outputPath);
		} catch (error) {
			if (!ended) await writer.end();
			await rm(partial, { force: true });
			throw error;
		} finally {
			await command("IO.close", { handle });
		}
		const result = await finished;
		return {
			bytes,
			dataLossOccurred: result.dataLossOccurred === true,
			stoppedBy,
			capturedSeconds,
		};
	}
	function stop(stoppedBy = "benchmark-complete") {
		stopPromise ??= save(stoppedBy);
		return stopPromise;
	}
	try {
		await command("Tracing.start", {
			transferMode: "ReturnAsStream",
			streamFormat: "json",
			traceConfig: {
				recordMode: "recordUntilFull",
				traceBufferSizeInKb: browserTraceBufferKb,
				includedCategories: browserTraceCategories,
				excludedCategories: ["*"],
			},
		});
		startedAt = performance.now();
		timer = setTimeout(() => {
			void stop("duration-limit").catch(() => undefined);
		}, maximumSeconds * 1000);
		const targets = await command("Target.getTargets");
		const page = (targets.targetInfos as { type: string; targetId: string }[]).find(
			(target) => target.type === "page",
		);
		if (!page) throw new Error("Isolated trace browser has no page target");
		const attached = await command("Target.attachToTarget", {
			targetId: page.targetId,
			flatten: true,
		});
		if (typeof attached.sessionId !== "string")
			throw new Error("Browser trace cannot attach the isolated page");
		const navigation = await command("Page.navigate", { url: url.href }, attached.sessionId);
		if (navigation.errorText)
			throw new Error(`Trace page navigation failed: ${navigation.errorText}`);
		return {
			stop,
			close() {
				clearTimeout(timer);
				socket.close();
			},
		};
	} catch (error) {
		clearTimeout(timer);
		socket.close();
		throw error;
	}
}
