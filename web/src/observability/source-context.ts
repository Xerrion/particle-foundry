import type { ErrorEvent } from "@sentry/browser";
import { type RawSourceMap, SourceMapConsumer } from "source-map-js";
import {
	annotateRustOrigin,
	attachSourceLink,
	type RustLocation,
	type SourceFrame,
} from "./rust-origin";

export interface SourceConfiguration {
	baseUrl?: string;
	revision?: string;
	fetch?: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
}

const MAX_ASSET_BYTES = 8 * 1024 * 1024;
const MAX_FRAMES = 50;

function authoredWebFile(source: string, mapUrl: URL): string | undefined {
	if (source.includes("node_modules") || source.includes("generated/")) return undefined;
	const path = decodeURIComponent(new URL(source, mapUrl).pathname);
	const match = /\/(?:web\/)?(src|tests|benchmarks)\/(.+\.tsx?)$/.exec(path);
	return match ? `web/${match[1]}/${match[2]}` : undefined;
}

function sourceContext(frame: SourceFrame, lines: string[], line: number): void {
	const index = line - 1;
	if (!lines[index]) return;
	const boundedLine = (value: string): string => value.slice(0, 500);
	frame.context_line = boundedLine(lines[index]);
	frame.pre_context = lines.slice(Math.max(0, index - 5), index).map(boundedLine);
	frame.post_context = lines.slice(index + 1, index + 6).map(boundedLine);
}

/** Resolves public build sources only when an error is reported. */
export function createSourceEnricher(config: SourceConfiguration) {
	const page = typeof location === "undefined" ? undefined : new URL(location.href);
	const base = config.baseUrl
		? new URL(config.baseUrl, page)
		: page
			? new URL("/", page)
			: undefined;
	const fetchAsset = config.fetch ?? globalThis.fetch;
	const maps = new Map<string, Promise<SourceMapConsumer | undefined>>();

	function sourceLink(frame: SourceFrame, file: string): void {
		attachSourceLink(frame, file, config.revision);
	}

	async function assetText(url: URL): Promise<string | undefined> {
		if (!base || url.origin !== base.origin || !url.pathname.startsWith(base.pathname)) return;
		const response = await fetchAsset(url, {
			credentials: "omit",
			referrerPolicy: "no-referrer",
			redirect: "error",
			signal: AbortSignal.timeout(1_500),
		});
		if (!response.ok || Number(response.headers.get("content-length")) > MAX_ASSET_BYTES) return;
		const reader = response.body?.getReader();
		if (!reader) return;
		const decoder = new TextDecoder();
		let text = "";
		let bytes = 0;
		try {
			while (true) {
				const chunk = await reader.read();
				if (chunk.done) break;
				bytes += chunk.value.byteLength;
				if (bytes > MAX_ASSET_BYTES) {
					await reader.cancel();
					return;
				}
				text += decoder.decode(chunk.value, { stream: true });
			}
			return text + decoder.decode();
		} finally {
			reader.releaseLock();
		}
	}

	async function loadMap(url: URL): Promise<SourceMapConsumer | undefined> {
		const text = await assetText(url);
		if (!text) return;
		const map: unknown = JSON.parse(text);
		if (typeof map !== "object" || map === null) return;
		const record = map as Record<string, unknown>;
		if (
			(record.version !== 3 && record.version !== "3") ||
			!Array.isArray(record.sources) ||
			!record.sources.every((value) => typeof value === "string") ||
			!Array.isArray(record.names) ||
			!record.names.every((value) => typeof value === "string") ||
			typeof record.mappings !== "string"
		)
			return;
		return new SourceMapConsumer(map as RawSourceMap);
	}

	async function mapFrame(frame: SourceFrame): Promise<boolean> {
		if (!base || !frame.filename || !frame.lineno || !frame.colno) return false;
		const url = new URL(frame.filename, base);
		if (
			url.origin !== base.origin ||
			!url.pathname.startsWith(base.pathname) ||
			!url.pathname.endsWith(".js")
		)
			return false;
		url.search = "";
		url.hash = "";
		url.pathname += ".map";
		let pending = maps.get(url.href);
		if (!pending) {
			if (maps.size >= 4) maps.delete(maps.keys().next().value ?? "");
			pending = loadMap(url)
				.catch(() => undefined)
				.then((consumer) => {
					if (!consumer && maps.get(url.href) === pending) maps.delete(url.href);
					return consumer;
				});
			maps.set(url.href, pending);
		}
		const consumer = await pending;
		if (!consumer) return false;
		// SDK line and column positions are one-based. Source maps use zero-based columns.
		const original = consumer.originalPositionFor({ line: frame.lineno, column: frame.colno - 1 });
		if (!original.source || !original.line) return false;
		const file = authoredWebFile(original.source, url);
		if (!file) {
			frame.in_app = false;
			return false;
		}
		frame.filename = file;
		delete frame.abs_path;
		frame.lineno = original.line;
		frame.colno = original.column + 1;
		if (original.name) frame.function = original.name;
		frame.in_app = true;
		const content = consumer.sourceContentFor(original.source, true);
		if (content) sourceContext(frame, content.split(/\r?\n/), original.line);
		sourceLink(frame, file);
		return true;
	}

	async function rustFrame(origin: RustLocation): Promise<SourceFrame> {
		const frame: SourceFrame = {
			filename: origin.file,
			lineno: origin.line,
			colno: origin.column,
			in_app: true,
			platform: "rust",
		};
		sourceLink(frame, origin.file);
		if (base) {
			const text = await assetText(new URL(`reporting-sources/${origin.file}.json`, base));
			if (text) {
				const source: unknown = JSON.parse(text);
				if (typeof source === "object" && source !== null) {
					const record = source as Record<string, unknown>;
					if (
						record.file === origin.file &&
						Array.isArray(record.lines) &&
						record.lines.every((line) => typeof line === "string")
					) {
						sourceContext(frame, record.lines, origin.line);
					}
				}
			}
		}
		return frame;
	}

	return {
		async enrich(event: ErrorEvent, error: unknown): Promise<void> {
			const origin = annotateRustOrigin(event, error, config.revision);
			let resolved = origin ? 1 : 0;
			const frames =
				event.exception?.values?.flatMap((exception) => exception.stacktrace?.frames ?? []) ?? [];
			const originFrame = origin
				? frames.find((frame) => frame.filename === origin.file && frame.lineno === origin.line)
				: undefined;
			await Promise.all([
				...frames.slice(-MAX_FRAMES).map(async (frame) => {
					try {
						if (await mapFrame(frame)) resolved++;
					} catch {
						/* Preserve the original frame when diagnostics fail. */
					}
				}),
				origin && originFrame
					? rustFrame(origin)
							.then((frame) => Object.assign(originFrame, frame))
							.catch(() => undefined)
					: Promise.resolve(),
			]);
			event.tags = { ...event.tags, source_frames: String(resolved) };
		},
	};
}
