/**
 * Video Merge State Management Composable
 *
 * ，。
 * ， TS  MP4/MKV ， composable 。
 *
 * Tracks session directories that are merging or waiting to merge, provides
 * merge progress queries and state management.
 * After recording ends, multiple TS segments are merged into a single MP4/MKV file;
 * this composable manages the state of that process.
 *
 * ： composable  useMergingStore ，
 * 。
 *
 * Note: This composable is now a thin wrapper around the global useMergingStore
 * to maintain interface compatibility with existing callers.
 */

import { useMergingStore } from "@/stores/merging";

/**
 * 。
 * Video merge state and operations.
 */
export function useMerging() {
	const store = useMergingStore();
	return {
		mergingDirs: store.mergingDirs,
		mergeProgress: store.mergeProgress,
		waitingMergeDirs: store.waitingMergeDirs,
		isMerging: store.isMerging,
		isWaitingMerge: store.isWaitingMerge,
		getMergeProgress: store.getMergeProgress,
		addMerging: store.addMerging,
		addWaitingMerge: store.addWaitingMerge,
		clearMergingForUsername: store.clearMergingForUsername,
		clearMergingForSessionDir: store.clearMergingForSessionDir,
		initFromBackend: store.initFromBackend,
	};
}
