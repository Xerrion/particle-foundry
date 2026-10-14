import { type FixtureName, fixtureNames } from "./fixtures";

export type ActivityOptions = {
	mode: "baseline" | "activity" | "profile";
	fixture: FixtureName;
	ticks: number;
	warmupTicks: number;
	repeats: number;
	sampleTick: number;
	samplePasses: number;
	sampleEvery: number;
};

export function readOptions(params: URLSearchParams): ActivityOptions {
	const mode = params.get("mode") ?? "baseline";
	const fixture = params.get("fixture") ?? "brush";
	if (mode !== "baseline" && mode !== "activity" && mode !== "profile")
		throw new Error("Invalid activity mode");
	if (!fixtureNames.includes(fixture as FixtureName)) throw new Error("Invalid activity fixture");
	function integer(key: string, fallback: number, minimum: number, maximum: number) {
		const value = Number(params.get(key) ?? fallback);
		if (!Number.isSafeInteger(value) || value < minimum || value > maximum)
			throw new Error(`Invalid ${key}`);
		return value;
	}
	const ticks = integer("ticks", 60, 1, 300);
	return {
		mode,
		fixture: fixture as FixtureName,
		ticks,
		warmupTicks: integer("warmup-ticks", 10, 0, 60),
		repeats: integer("repeats", 3, 1, 10),
		sampleTick: integer("sample-tick", Math.min(10, ticks - 1), 0, ticks - 1),
		samplePasses: integer("sample-passes", 2, 1, 8),
		sampleEvery: integer("sample-every", 64, 1, 1024),
	};
}

export function reporter(params: URLSearchParams) {
	const token = params.get("report-token");
	const rawEndpoint = params.get("report-url");
	if (!token || !/^[a-f0-9-]{36}$/i.test(token) || !rawEndpoint)
		throw new Error("Missing activity report receiver");
	const endpoint = new URL(rawEndpoint);
	if (
		endpoint.hostname !== "127.0.0.1" ||
		endpoint.protocol !== "http:" ||
		endpoint.pathname !== "/report"
	) {
		throw new Error("Activity reports require a loopback receiver");
	}
	async function send(value: object) {
		const response = await fetch(endpoint, {
			method: "POST",
			body: JSON.stringify({ token, ...value }),
		});
		if (!response.ok)
			throw new Error(`Activity receiver rejected a report: HTTP ${response.status}`);
	}
	return {
		report: (report: unknown) => send({ kind: "result", report }),
		async checkpoint(name: string, bytes: Uint8Array) {
			let binary = "";
			for (let index = 0; index < bytes.length; index += 8192) {
				binary += String.fromCharCode(...bytes.subarray(index, index + 8192));
			}
			await send({ kind: "checkpoint", name, bytes: btoa(binary) });
		},
	};
}
