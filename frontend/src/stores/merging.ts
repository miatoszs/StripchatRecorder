/**
 * Video Merge State Management Store
 *
 * Pinia store，（）
 * 。
 *
 * Elevates merge state to a global Pinia store so that other views
 * (e.g. the streamer list) can clean up merge queue state when a streamer is removed.
 */

import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { call } from "@/lib/api";

export const useMergingStore = defineStore("merging", () => {
	/*Map of merging session dir -> target file path */
	const mergingDirs = ref<Map<string, string>>(new Map());

	/*total bytes) */
	const mergeProgress = ref<
		Record<string, { out_bytes: number; total_bytes: number }>
	>({});

	/*Map of waiting-to-merge session dir -> target file path */
	const waitingMergeDirs = ref<Map<string, string>>(new Map());

	/** （）/ Set of target paths currently merging (normalized to forward slashes) */
	const mergingTargetPaths = computed(
		() =>
			new Set(
				[...mergingDirs.value.values()].map((p) => p.replace(/\\/g, "/")),
			),
	);

	/*Set of target paths waiting to merge */
	const waitingMergeTargetPaths = computed(
		() =>
			new Set(
				[...waitingMergeDirs.value.values()].map((p) => p.replace(/\\/g, "/")),
			),
	);

	/**
	 * （）。
	 * Check if the file at the given path is currently merging (including waiting).
	 */
	function isMerging(path: string): boolean {
		const norm = path.replace(/\\/g, "/");
		return (
			mergingTargetPaths.value.has(norm) ||
			waitingMergeTargetPaths.value.has(norm)
		);
	}

	/**
	 * 。
	 * Check if the file at the given path is in the waiting-to-merge queue.
	 */
	function isWaitingMerge(path: string): boolean {
		return waitingMergeTargetPaths.value.has(path.replace(/\\/g, "/"));
	}

	/**
	 * （0-99）， null。
	 * Get the merge progress percentage (0-99) for a target file, returns null if not found.
	 */
	function getMergeProgress(targetPath: string): number | null {
		const norm = targetPath.replace(/\\/g, "/");
		for (const [sessionDir, tp] of mergingDirs.value) {
			if (tp.replace(/\\/g, "/") === norm) {
				const p = mergeProgress.value[sessionDir];
				if (!p || p.total_bytes === 0) return 0;
				return Math.min(
					99,
					Math.floor((p.out_bytes / p.total_bytes) * 10000) / 100,
				);
			}
		}
		return null;
	}

	/**
	 * ，。
	 * Mark a session directory as actively merging and remove it from the waiting queue.
	 */
	function addMerging(sessionDir: string, mergedPath: string) {
		mergingDirs.value = new Map(mergingDirs.value).set(sessionDir, mergedPath);
		mergeProgress.value[sessionDir] = { out_bytes: 0, total_bytes: 0 };
		const next = new Map(waitingMergeDirs.value);
		next.delete(sessionDir);
		waitingMergeDirs.value = next;
	}

	/**
	 * 。
	 * Add a session directory to the waiting-to-merge queue.
	 */
	function addWaitingMerge(sessionDir: string, mergedPath: string) {
		waitingMergeDirs.value = new Map(waitingMergeDirs.value).set(
			sessionDir,
			mergedPath,
		);
	}

	/**
	 * （）。
	 * Clear the merge state for a specific session directory (called after merge completes or fails).
	 */
	function clearMergingForSessionDir(sessionDir: string) {
		const nextMerging = new Map(mergingDirs.value);
		nextMerging.delete(sessionDir);
		mergingDirs.value = nextMerging;
		delete mergeProgress.value[sessionDir];
		const nextWaiting = new Map(waitingMergeDirs.value);
		nextWaiting.delete(sessionDir);
		waitingMergeDirs.value = nextWaiting;
	}

	/**
	 * （）。
	 * Clear all merge states for a specific streamer (called when a streamer is removed).
	 *
	 * Streamer username
	 */
	function clearMergingForUsername(username: string) {
		const next = new Map(mergingDirs.value);
		for (const [dir] of next) {
			const parts = dir.split(/[\\/]/).filter(Boolean);
			if (parts.slice(-2, -1)[0] === username) next.delete(dir);
		}
		mergingDirs.value = next;
		for (const dir of Object.keys(mergeProgress.value)) {
			if (!next.has(dir)) delete mergeProgress.value[dir];
		}
		const nextWaiting = new Map(waitingMergeDirs.value);
		for (const [dir] of nextWaiting) {
			const parts = dir.split(/[\\/]/).filter(Boolean);
			if (parts.slice(-2, -1)[0] === username) nextWaiting.delete(dir);
		}
		waitingMergeDirs.value = nextWaiting;
	}

	/**
	 * （）。
	 * Restore merge state from the backend (called after page refresh or reconnect).
	 */
	async function initFromBackend() {
		try {
			const merging = await call<
				{
					session_dir: string;
					merged_path: string;
					merge_format: string;
					username: string;
					status?: string;
				}[]
			>("get_merging_dirs");
			const nextMerging = new Map(mergingDirs.value);
			const nextWaiting = new Map(waitingMergeDirs.value);
			for (const m of merging) {
				if (m.status === "waiting") {
					nextWaiting.set(m.session_dir, m.merged_path);
				} else {
					nextMerging.set(m.session_dir, m.merged_path);
					mergeProgress.value[m.session_dir] = { out_bytes: 0, total_bytes: 0 };
				}
			}
			mergingDirs.value = nextMerging;
			waitingMergeDirs.value = nextWaiting;
		} catch {
			console.log("Failed to get merging dirs from backend");
		}
	}

	return {
		mergingDirs,
		mergeProgress,
		waitingMergeDirs,
		isMerging,
		isWaitingMerge,
		getMergeProgress,
		addMerging,
		addWaitingMerge,
		clearMergingForUsername,
		clearMergingForSessionDir,
		initFromBackend,
	};
});
