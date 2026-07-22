/**
 * Post-processing Task Status Store
 *
 * 、 Pinia store，
 * （）。
 *
 * Elevates post-processing task status, progress, and module output paths to a
 * global Pinia store so that other views (e.g. the streamer list) can cancel and
 * clean up post-processing tasks when a streamer is removed.
 */

import { defineStore } from "pinia";
import { ref } from "vue";
import type { PpStatus, PpProgress } from "@/composables/usePostprocess";
import { call } from "@/lib/api";

export const usePpStatusStore = defineStore("ppStatus", () => {
	/*Post-processing status per file path */
	const ppStatus = ref<Record<string, PpStatus>>({});

	/*Post-processing progress per file path */
	const ppProgress = ref<Record<string, PpProgress>>({});

	/*Module output paths per file path */
	const moduleOutputs = ref<Record<string, Record<string, string>>>({});

	/**
	 * （）。
	 * Clear all post-processing state for a specific file (called when file is deleted).
	 */
	function removeFile(path: string) {
		delete ppStatus.value[path];
		delete ppProgress.value[path];
		delete moduleOutputs.value[path];
	}

	/**
	 * （）。
	 * Cancel and clear all post-processing tasks for a specific streamer (called when a streamer is removed).
	 *
	 * 。
	 * Identifies files belonging to the streamer by the second-to-last path segment.
	 *
	 * Streamer username
	 */
	async function cancelAndClearForUsername(username: string) {
		const pathsToRemove = Object.keys(ppStatus.value).filter((path) => {
			const parts = path.split(/[\\/]/).filter(Boolean);
			return parts.slice(-2, -1)[0] === username;
		});

		// Cancel all running tasks concurrently
		await Promise.all(
			pathsToRemove
				.filter(
					(p) =>
						ppStatus.value[p] === "running" ||
						ppStatus.value[p] === "waiting",
				)
				.map((p) => call("cancel_postprocess", { path: p }).catch(() => {})),
		);

		// Clear frontend state
		for (const path of pathsToRemove) {
			removeFile(path);
		}
	}

	return {
		ppStatus,
		ppProgress,
		moduleOutputs,
		removeFile,
		cancelAndClearForUsername,
	};
});
