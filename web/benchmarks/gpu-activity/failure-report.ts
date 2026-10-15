import type { BrowserGpuSceneSession } from "../../src/engine-client/wasm";
import type { installProfile } from "./profile";
import type { PressureJournal } from "./timeline";

type FailureDiagnostics = {
	scene?: Pick<
		BrowserGpuSceneSession,
		"take_pressure_diagnostics_json" | "disable_pressure_diagnostics"
	>;
	profile?: Pick<ReturnType<typeof installProfile>, "stop" | "finish" | "snapshot">;
	pressureRecordingEnabled: boolean;
	pressureJournal?: PressureJournal;
};

/** Collect the recorded prefix without replacing the failure that stopped the fixture. */
export async function failedProfileReport(error: unknown, diagnostics: FailureDiagnostics) {
	const exportErrors: { stage: string; error: string }[] = [];
	let pressureJournal = diagnostics.pressureJournal;
	let pressureStatus = pressureJournal ? "retained" : "not-enabled";
	let profiling: ReturnType<ReturnType<typeof installProfile>["snapshot"]> | undefined;
	let profileStatus = diagnostics.profile ? "partial" : "unavailable";
	diagnostics.profile?.stop();
	if (diagnostics.pressureRecordingEnabled && diagnostics.scene) {
		try {
			pressureJournal = JSON.parse(
				diagnostics.scene.take_pressure_diagnostics_json(),
			) as PressureJournal;
			pressureStatus = "collected";
		} catch (error) {
			pressureStatus = "error";
			exportErrors.push({ stage: "pressure-journal", error: String(error) });
		} finally {
			try {
				diagnostics.scene.disable_pressure_diagnostics();
			} catch (error) {
				exportErrors.push({ stage: "disable-pressure-journal", error: String(error) });
			}
		}
	}
	if (diagnostics.profile) {
		let timeout: ReturnType<typeof setTimeout> | undefined;
		try {
			profiling = await Promise.race([
				diagnostics.profile.finish(),
				new Promise<never>((_, reject) => {
					timeout = setTimeout(
						() => reject(new Error("Failed-run timestamp collection exceeded 5 seconds")),
						5000,
					);
				}),
			]);
			profileStatus = "complete";
		} catch (error) {
			exportErrors.push({ stage: "profile-timestamps", error: String(error) });
			profiling = diagnostics.profile.snapshot();
		} finally {
			clearTimeout(timeout);
		}
	}
	return {
		status: "fail",
		error: error instanceof Error ? (error.stack ?? error.message) : String(error),
		profile: profiling,
		pressureJournal,
		diagnosticExport: {
			partial: true,
			profileStatus,
			pressureStatus,
			errors: exportErrors,
			note: "The fixture failed. Recorded prefixes remain diagnostic evidence, not a completed acceptance run. Uncollected timestamp batches and dropped-event counts remain explicit.",
		},
	};
}
