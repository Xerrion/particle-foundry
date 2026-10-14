export interface PaintPoint {
	x: number;
	y: number;
}

export interface PaintBrush {
	radius: number;
	material: "water" | "air";
}

interface PaintPointerOptions {
	available(): boolean;
	begin(point: PaintPoint): PaintBrush | undefined;
	segment(from: PaintPoint, to: PaintPoint, brush: PaintBrush): boolean;
}

/** Connects accepted pointer samples without extending strokes across unavailable state. */
export function bindGpuPaintPointer(canvas: HTMLCanvasElement, options: PaintPointerOptions) {
	let stroke: { pointerId: number; last: PaintPoint; brush: PaintBrush } | undefined;

	function cancel(): void {
		const previous = stroke;
		stroke = undefined;
		if (previous && canvas.hasPointerCapture(previous.pointerId)) {
			canvas.releasePointerCapture(previous.pointerId);
		}
	}

	function pointAt(event: PointerEvent): PaintPoint | undefined {
		const bounds = canvas.getBoundingClientRect();
		if (bounds.width <= 0 || bounds.height <= 0) return;
		const x = Math.floor(((event.clientX - bounds.left) / bounds.width) * canvas.width);
		const y = Math.floor(((event.clientY - bounds.top) / bounds.height) * canvas.height);
		if (x < 0 || x >= canvas.width || y < 0 || y >= canvas.height) return;
		return { x, y };
	}

	function extend(event: PointerEvent): void {
		const current = stroke;
		if (!current || current.pointerId !== event.pointerId) return;
		const to = pointAt(event);
		if (!options.available() || !to) {
			cancel();
			return;
		}
		event.preventDefault();
		if (to.x === current.last.x && to.y === current.last.y) return;
		if (options.segment(current.last, to, current.brush)) current.last = to;
		else cancel();
	}

	canvas.addEventListener("pointerdown", (event) => {
		if (stroke || event.button !== 0 || !event.isPrimary || !options.available()) return;
		const point = pointAt(event);
		if (!point) return;
		event.preventDefault();
		const brush = options.begin(point);
		if (!brush) return;
		stroke = { pointerId: event.pointerId, last: point, brush: { ...brush } };
		canvas.setPointerCapture(event.pointerId);
	});
	canvas.addEventListener("pointermove", (event) => {
		if (stroke?.pointerId !== event.pointerId) return;
		if ((event.buttons & 1) === 0) cancel();
		else extend(event);
	});
	canvas.addEventListener("pointerup", (event) => {
		if (stroke?.pointerId !== event.pointerId) return;
		extend(event);
		cancel();
	});
	for (const name of ["pointercancel", "lostpointercapture"] as const) {
		canvas.addEventListener(name, (event) => {
			if (stroke?.pointerId === event.pointerId) cancel();
		});
	}
	return { cancel };
}
