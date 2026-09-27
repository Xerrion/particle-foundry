const claimed = new WeakMap<HTMLCanvasElement, "2d" | "webgpu">();

/** Chooses a canvas API before either backend acquires its context. */
export function selectCanvasContext(
	canvas: HTMLCanvasElement,
	kind: "2d",
): CanvasRenderingContext2D;
export function selectCanvasContext(canvas: HTMLCanvasElement, kind: "webgpu"): unknown;
export function selectCanvasContext(canvas: HTMLCanvasElement, kind: "2d" | "webgpu"): unknown {
	const previous = claimed.get(canvas);
	if (previous && previous !== kind) throw new Error(`Canvas already belongs to ${previous}`);
	const context =
		kind === "2d" ? canvas.getContext("2d", { alpha: false }) : canvas.getContext("webgpu");
	if (!context) throw new Error(`${kind} canvas context is unavailable`);
	claimed.set(canvas, kind);
	return context;
}
