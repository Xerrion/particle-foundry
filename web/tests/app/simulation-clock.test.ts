import { describe, expect, test } from "bun:test";
import { createSimulationClock } from "../../src/app/simulation-clock";

const STEP_MILLISECONDS = 1000 / 60;
const SPEEDS = [0.5, 1, 2, 4];

describe("elapsed-time simulation clock", () => {
	for (const speed of SPEEDS) {
		for (const refreshRate of [30, 60, 144, 240]) {
			test(`${speed}x at ${refreshRate} Hz produces the same steps over ten seconds`, () => {
				const clock = createSimulationClock();
				let previousTimestamp = 0;
				let steps = 0;
				for (let frame = 1; frame <= refreshRate * 10; frame += 1) {
					const timestamp = (frame * 1000) / refreshRate;
					steps += clock.advance(timestamp - previousTimestamp, {
						speed,
						isPaused: false,
					});
					previousTimestamp = timestamp;
				}
				expect(steps).toBe(600 * speed);
				expect(clock.advance(0, { speed, isPaused: false })).toBe(0);
			});
		}

		test(`${speed}x retains time through jitter and zero-length frames`, () => {
			const clock = createSimulationClock();
			let steps = 0;
			for (let cycle = 0; cycle < 100; cycle += 1) {
				for (const elapsed of [3, 27, 11, 0, 42, 17]) {
					steps += clock.advance(elapsed, { speed, isPaused: false });
				}
			}
			expect(steps).toBe(600 * speed);
		});

		test(`${speed}x bounds long gaps without queuing discarded time or losing the fraction`, () => {
			for (const elapsed of [100, 101, 1000, 60_000, Number.MAX_VALUE]) {
				const clock = createSimulationClock();
				expect(clock.advance(1, { speed, isPaused: false })).toBe(0);
				expect(clock.advance(elapsed, { speed, isPaused: false })).toBe(6 * speed);
				expect(clock.advance(0, { speed, isPaused: false })).toBe(0);
				expect(
					clock.advance(STEP_MILLISECONDS / speed - 1, {
						speed,
						isPaused: false,
					}),
				).toBe(1);
			}
		});

		test(`${speed}x discards paused time and starts a fresh fraction on resume`, () => {
			const clock = createSimulationClock();
			const halfStep = STEP_MILLISECONDS / speed / 2;
			expect(clock.advance(halfStep, { speed, isPaused: false })).toBe(0);
			for (const elapsed of [0, 17, 60_000]) {
				expect(clock.advance(elapsed, { speed, isPaused: true })).toBe(0);
			}
			expect(clock.advance(halfStep, { speed, isPaused: false })).toBe(0);
			expect(clock.advance(halfStep, { speed, isPaused: false })).toBe(1);
		});
	}

	test("half speed retains alternating steps beyond repeated FPS sampling intervals", () => {
		const clock = createSimulationClock();
		for (let frame = 1; frame <= 1200; frame += 1) {
			expect(clock.advance(STEP_MILLISECONDS, { speed: 0.5, isPaused: false })).toBe(
				frame % 2 === 0 ? 1 : 0,
			);
		}
	});

	test("speed changes retain already accumulated simulation time", () => {
		const clock = createSimulationClock();
		for (const speed of SPEEDS) {
			expect(
				clock.advance(STEP_MILLISECONDS / speed / 4, {
					speed,
					isPaused: false,
				}),
			).toBe(speed === 4 ? 1 : 0);
		}
	});

	test("zero elapsed preserves the fraction without creating steps", () => {
		const clock = createSimulationClock();
		expect(clock.advance(0, { speed: 1, isPaused: false })).toBe(0);
		expect(clock.advance(10, { speed: 1, isPaused: false })).toBe(0);
		for (let frame = 0; frame < 100; frame += 1) {
			expect(clock.advance(0, { speed: 4, isPaused: false })).toBe(0);
		}
		expect(clock.advance(STEP_MILLISECONDS - 10, { speed: 1, isPaused: false })).toBe(1);
	});

	test("rounding debt never returns negative steps or creates time on tiny frames", () => {
		const clock = createSimulationClock();
		expect(
			clock.advance(STEP_MILLISECONDS * (1 - 1e-9), {
				speed: 1,
				isPaused: false,
			}),
		).toBe(1);
		for (const elapsed of [0, Number.MIN_VALUE, 1e-10]) {
			expect(clock.advance(elapsed, { speed: 1, isPaused: false })).toBe(0);
		}
		expect(clock.advance(STEP_MILLISECONDS, { speed: 1, isPaused: false })).toBe(1);
	});

	test("reset discards the fraction and is safe before use or repeatedly", () => {
		const clock = createSimulationClock();
		clock.reset();
		expect(clock.advance(10, { speed: 1, isPaused: false })).toBe(0);
		clock.reset();
		clock.reset();
		expect(clock.advance(10, { speed: 1, isPaused: false })).toBe(0);
		expect(clock.advance(10, { speed: 1, isPaused: false })).toBe(1);
	});

	test("separate clocks cannot share fractions or resets", () => {
		const first = createSimulationClock();
		const second = createSimulationClock();
		expect(first.advance(10, { speed: 1, isPaused: false })).toBe(0);
		expect(second.advance(10, { speed: 1, isPaused: false })).toBe(0);
		first.reset();
		expect(first.advance(10, { speed: 1, isPaused: false })).toBe(0);
		expect(second.advance(10, { speed: 1, isPaused: false })).toBe(1);
	});

	for (const isPaused of [false, true]) {
		test(`invalid elapsed time is rejected before mutation (paused: ${isPaused})`, () => {
			for (const elapsed of [-1, NaN, Infinity, -Infinity]) {
				const clock = createSimulationClock();
				expect(clock.advance(10, { speed: 1, isPaused: false })).toBe(0);
				expect(() => clock.advance(elapsed, { speed: 1, isPaused })).toThrow(RangeError);
				expect(clock.advance(STEP_MILLISECONDS - 10, { speed: 1, isPaused: false })).toBe(1);
			}
		});

		test(`invalid speed is rejected before mutation (paused: ${isPaused})`, () => {
			for (const speed of [
				0,
				-1,
				0.25,
				0.75,
				1.5,
				3,
				5,
				NaN,
				Infinity,
				-Infinity,
				Number.MAX_VALUE,
			]) {
				const clock = createSimulationClock();
				expect(clock.advance(10, { speed: 1, isPaused: false })).toBe(0);
				expect(() => clock.advance(0, { speed, isPaused })).toThrow(RangeError);
				expect(clock.advance(STEP_MILLISECONDS - 10, { speed: 1, isPaused: false })).toBe(1);
			}
		});
	}
});
