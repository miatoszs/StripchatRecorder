/**
 * Post-processing Task Management Composable
 *
 * ，：
 * - （////）
 * -
 * - （ contact_sheet ）
 * -
 *
 * Manages post-processing pipeline execution state and progress for recording files, including:
 * - Task status tracking (idle/waiting/running/done/error)
 * - Overall and per-module progress calculation
 * - Module output path inference (e.g., contact_sheet preview image path)
 * - Restoring task state from backend after page refresh
 */

import { call } from "@/lib/api";
import { usePostprocessStore } from "@/stores/postprocess";
import { usePpStatusStore } from "@/stores/ppStatus";
import { storeToRefs } from "pinia";
import { useNotify } from "./useNotify";
import { useI18n } from "vue-i18n";

/*Post-processing task status */
export type PpStatus = "idle" | "waiting" | "running" | "done" | "error";

/*Post-processing progress information */
export interface PpProgress {
	/*Number of completed modules */
	overallDone: number;
	/*Total number of modules */
	overallTotal: number;
	/*Overall progress percentage */
	overallPct: number;
	/*Overall progress label text */
	overallLabel: string;
	/*Current module done progress value */
	moduleDone: number;
	/*Current module total progress value */
	moduleTotal: number;
	/*Current module progress percentage */
	modulePct: number;
	/*Current module progress label text */
	moduleLabel: string;
	/*Current module name */
	moduleName: string;
	/** （ "2/3"）/ Module execution index label (e.g. "2/3") */
	moduleExecLabel: string;
	/*Full display text for current module */
	currentModuleText: string;
	/**
	 * （， postprocess-done  meta pp_results）。
	 * Per-module execution results (filled after completion, from postprocess-done event or meta pp_results).
	 */
	moduleResults?: { moduleId: string; success: boolean; message: string }[];
}

/**
 * [0, 100] 。
 * Clamp a percentage value to [0, 100] with two decimal places.
 */
function clampPct2(value: number): number {
	if (!Number.isFinite(value)) return 0;
	return Math.min(100, Math.max(0, Math.round(value * 100) / 100));
}

/**
 * （ "42.50%"）。
 * Format a percentage value as a string with two decimal places (e.g. "42.50%").
 */
function formatPct2(value: number): string {
	return `${clampPct2(value).toFixed(2)}%`;
}

/*i18n labels passed to makePpProgress */
export interface PpProgressLabels {
	/*Placeholder when module name is empty */
	processing: string;
	/*Label when no progress data is available */
	waiting: string;
}

const DEFAULT_LABELS: PpProgressLabels = {
	processing: "processing",
	waiting: "waiting",
};

/**
 * PpProgress 。
 * Build a PpProgress object from overall and module progress values.
 *
 * Number of completed modules
 * Total number of modules
 * Current module done progress
 * Current module total progress
 * Current module name
 * @param overallPctFallback - （）/ Fallback overall percentage (from backend)
 * @param prevModuleName - （）/ Previous module name (for regression prevention)
 * @param prevModulePct - （）/ Previous module progress (for regression prevention)
 * i18n labels
 */
export function makePpProgress(
	overallDone: number,
	overallTotal: number,
	moduleDone: number,
	moduleTotal: number,
	moduleName: string,
	overallPctFallback = 0,
	prevModuleName = "",
	prevModulePct = 0,
	labels: PpProgressLabels = DEFAULT_LABELS,
): PpProgress {
	const overallPctByNode =
		overallTotal > 0 ? clampPct2((overallDone * 100) / overallTotal) : 0;
	// ，
	// Take the larger of node-calculated and backend-reported values to prevent progress regression
	const overallPct =
		overallTotal > 0
			? Math.max(overallPctByNode, clampPct2(overallPctFallback))
			: clampPct2(overallPctFallback);

	const hasModuleProgress = moduleTotal > 0;
	const rawModulePct = hasModuleProgress
		? clampPct2((moduleDone * 100) / moduleTotal)
		: 0;
	// ； 0
	// Prevent regression within the same module; allow reset to 0 on module switch
	const isSameModule =
		moduleName.trim() === prevModuleName.trim() && moduleName.trim() !== "";
	const modulePct = isSameModule
		? Math.max(rawModulePct, prevModulePct)
		: rawModulePct;

	// （1-based）
	// Calculate the current executing module index (1-based)
	let moduleExecLabel = "";
	if (overallTotal > 0) {
		const moduleIndex = hasModuleProgress
			? Math.min(overallTotal, overallDone + 1)
			: Math.min(overallTotal, Math.max(1, overallDone));
		moduleExecLabel = `${moduleIndex}/${overallTotal}`;
	}

	const normalizedModuleName = moduleName.trim() || labels.processing;

	return {
		overallDone,
		overallTotal,
		overallPct,
		overallLabel: formatPct2(overallPct),
		moduleDone,
		moduleTotal,
		modulePct,
		moduleLabel: hasModuleProgress ? formatPct2(modulePct) : labels.waiting,
		moduleName: normalizedModuleName,
		moduleExecLabel,
		currentModuleText: moduleExecLabel
			? `${moduleExecLabel} ${normalizedModuleName}`
			: normalizedModuleName,
	};
}

/**
 * 。
 * Post-processing task state and operations.
 */
export function usePostprocess() {
	const ppStore = usePostprocessStore();
	const ppStatusStore = usePpStatusStore();
	const { toast } = useNotify();
	const { t } = useI18n();

	/*i18n labels passed to makePpProgress */
	const ppLabels = (): PpProgressLabels => ({
		processing: t("usePostprocess.processing"),
		waiting: t("usePostprocess.waitingProgress"),
	});

	/** （ store）/ Post-processing status per file path (from global store) */
	const { ppStatus, ppProgress, moduleOutputs } = storeToRefs(ppStatusStore);

	/**
	 * （）。
	 * Infer module output paths from the current pipeline config (without requesting backend).
	 *
	 * Video file path
	 * Map of module ID -> output path
	 */
	function inferModuleOutputs(videoPath: string): Record<string, string> {
		const outputs: Record<string, string> = {};
		const pipeline = ppStore.pipeline;
		if (!pipeline?.nodes) return outputs;
		// Handle both Windows and Unix path separators
		const sep = videoPath.includes("\\") ? "\\" : "/";
		const parts = videoPath.split(sep);
		const filename = parts[parts.length - 1];
		const dir = parts.slice(0, -1).join(sep);
		const stem = filename.includes(".")
			? filename.slice(0, filename.lastIndexOf("."))
			: filename;
		for (const node of pipeline.nodes) {
			if (!node.enabled) continue;
			// contact_sheet ：
			// contact_sheet module: outputs an image file with the same name as the video
			if (node.moduleId === "contact_sheet") {
				const format = (node.params?.format as string) ?? "webp";
				outputs["contact_sheet"] = `${dir}${sep}${stem}.${format}`;
			}
		}
		return outputs;
	}

	/**
	 * 。
	 * Fetch module output paths for a specific file from the backend.
	 *
	 * Video file path
	 */
	async function fetchModuleOutputs(path: string) {
		try {
			const result = await call<Record<string, string>>("get_module_outputs", {
				path,
			});
			if (result && Object.keys(result).length > 0) {
				moduleOutputs.value = { ...moduleOutputs.value, [path]: result };
			}
		} catch {
			toast(t("usePostprocess.fetchOutputFailed"), "error");
		}
	}

	/**
	 * 。
	 * Trigger post-processing pipeline execution for a specific file.
	 *
	 * Video file path
	 */
	async function runPostprocess(path: string) {
		ppStatus.value[path] = "running";
		ppProgress.value[path] = makePpProgress(0, 0, 0, 0, "", 0, "", 0, ppLabels());
		try {
			await call("run_postprocess_cmd", { path });
		} catch (e) {
			ppStatus.value[path] = "error";
			delete ppProgress.value[path];
			toast(String(e), "error");
		}
	}

	/**
	 * （ SSE ）。
	 * /；done/error  list_recordings  meta 。
	 *
	 * Restore all post-processing task states from the backend (called after page refresh or SSE reconnect).
	 * Only restores running/waiting transient tasks; done/error status is handled by meta fields from list_recordings.
	 */
	async function restoreFromBackend() {
		try {
			const tasks = await call<
				{
					path: string;
					pct: number;
					modDone: number;
					modTotal: number;
					moduleName: string;
					done: number;
					total: number;
					status: string;
					fromMemory: boolean;
				}[]
			>("get_postprocess_tasks");
			for (const t of tasks) {
				// /
				// Only restore in-memory running/waiting tasks
				if (!t.fromMemory) continue;
				if (t.status === "waiting") {
					// （running），
					// Don't downgrade if a newer status (running) is already set
					if (ppStatus.value[t.path] !== "running") {
						ppStatus.value[t.path] = "waiting";
					}
				} else if (t.status === "running") {
					ppStatus.value[t.path] = t.status as PpStatus;
					ppProgress.value[t.path] = makePpProgress(
						t.done,
						t.total,
						t.modDone,
						t.modTotal,
						t.moduleName,
						t.pct,
						"",
						0,
						ppLabels(),
					);
				}
			}
		} catch {
			toast(t("usePostprocess.fetchTasksFailed"), "error");
		}
	}

	/**
	 * ，。
	 * Handle post-processing done event, update state and trigger file list reload.
	 *
	 * Done event data from backend
	 * File list reload callback
	 * @param isFileDeleted - （ true  toast ）/ Whether the file was deleted by the user (skip toast if true)
	 */
	function handlePostprocessDone(
		payload: {
			path: string;
			results: { moduleId: string; success: boolean; message: string }[];
		},
		onLoad: () => Promise<void>,
		isFileDeleted?: () => boolean,
	) {
		const allOk = payload.results.every((r) => r.success);
		ppStatus.value[payload.path] = allOk ? "done" : "error";

		// ， toast ，
		// If the file was deleted by the user, skip all toasts and just reload
		const deleted = isFileDeleted?.() ?? false;

		if (allOk) {
			// ： 100%
			// All modules succeeded: set progress to 100% and collect output paths
			ppProgress.value[payload.path] = {
				...makePpProgress(
					payload.results.length,
					payload.results.length,
					0,
					0,
					"",
					100,
					"",
					0,
					ppLabels(),
				),
				moduleResults: payload.results,
			};
			if (!deleted) {
				const names = payload.results.map((r) => r.moduleId).join(" → ");
				toast(t("usePostprocess.done", { modules: names }), "success");
			}
			// NodeResult.output  #[serde(skip)] ，。
			// inferModuleOutputs （ contact_sheet ）。
			// NodeResult.output is skipped during serialization (#[serde(skip)]), so the frontend
			// cannot access it directly. Use inferModuleOutputs to derive output paths from the
			// pipeline config (e.g., the contact_sheet image path).
			const inferred = inferModuleOutputs(payload.path);
			if (Object.keys(inferred).length > 0) {
				moduleOutputs.value = {
					...moduleOutputs.value,
					[payload.path]: inferred,
				};
			} else {
				fetchModuleOutputs(payload.path);
			}
		} else {
			ppProgress.value[payload.path] = {
				...makePpProgress(0, payload.results.length, 0, 0, "", 0, "", 0, ppLabels()),
				moduleResults: payload.results,
			};
			if (!deleted) {
				const failed = payload.results.find((r) => !r.success);
				toast(
					t("usePostprocess.failed", {
						moduleId: failed?.moduleId,
						message: failed?.message,
					}),
					"error",
				);
			}
		}
		return onLoad();
	}

	/**
	 * （）。
	 * Clear all post-processing state for a specific file (called when file is deleted).
	 *
	 * Video file path
	 */
	function removeFile(path: string) {
		ppStatusStore.removeFile(path);
	}

	return {
		ppStatus,
		ppProgress,
		moduleOutputs,
		inferModuleOutputs,
		fetchModuleOutputs,
		runPostprocess,
		restoreFromBackend,
		handlePostprocessDone,
		removeFile,
	};
}
