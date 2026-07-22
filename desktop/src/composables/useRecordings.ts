/**
 * Recording File Management Composable
 *
 * 、、、。
 * ，，。
 * （//） meta  `status` 。
 *
 * Manages loading, grouping, sorting, selection, and timing of recording files.
 * Files are grouped by streamer username, support multi-column sorting,
 * and provide a real-time timer for actively recording files.
 * All status (recording/merging/post-processing etc.) comes from the `status` field in backend meta files.
 */

import { ref, computed } from "vue";
import { call } from "@/lib/api";
import type { RecordingFile } from "@/types/recordings";
import { ArrowUpDown, ArrowUp, ArrowDown } from "@lucide/vue";

/*Supported sort keys */
export type SortKey = "started_at" | "size_bytes" | "video_duration_secs";
/*Sort direction */
export type SortDir = "asc" | "desc";

/*Recording file group by streamer */
export interface Group {
	username: string;
	files: RecordingFile[];
	/** （）/ Total size of all files in the group (bytes) */
	totalSize: number;
	/*Whether any file in the group is currently recording */
	hasRecording: boolean;
}

/**
 * 。
 * `{username}_{YYYYMMDD}_{HHmmss}.ext`。
 *
 * Extract the streamer username from a recording filename.
 * Filename format: `{username}_{YYYYMMDD}_{HHmmss}.ext`
 */
export function usernameFromFile(f: RecordingFile): string {
	const stem = f.name.replace(/\.[^.]+$/, "");
	const parts = stem.split("_");
	return parts.slice(0, -2).join("_");
}

/**
 * 。
 */
export function useRecordings() {
	/*All recording files */
	const files = ref<RecordingFile[]>([]);
	/*Whether loading */
	const loading = ref(false);
	/** （，）/ Elapsed recording duration per file (seconds, increments in real-time) */
	const elapsed = ref<Record<string, number>>({});
	/*Set of selected file paths */
	const selected = ref<Set<string>>(new Set());
	/*Set of collapsed group usernames */
	const collapsedGroups = ref<Set<string>>(new Set());
	/*Current sort key */
	const sortKey = ref<SortKey>("started_at");
	/*Current sort direction */
	const sortDir = ref<SortDir>("desc");

	/*Timer handle: increments recording duration every second */
	let tickTimer: ReturnType<typeof setInterval> | null = null;
	/*Timer handle: debounced file list refresh */
	let dirRefreshTimer: ReturnType<typeof setTimeout> | null = null;

	function toggleSort(key: SortKey) {
		if (sortKey.value === key) {
			sortDir.value = sortDir.value === "desc" ? "asc" : "desc";
		} else {
			sortKey.value = key;
			sortDir.value = "desc";
		}
	}

	function sortIcon(key: SortKey) {
		if (sortKey.value !== key) return ArrowUpDown;
		return sortDir.value === "desc" ? ArrowDown : ArrowUp;
	}

	/**
	 * （）。
	 * ，。
	 *
	 * Files grouped by streamer (computed property).
	 * Groups directly by username extracted from filename, no merging state filtering needed.
	 */
	const groups = computed<Group[]>(() => {
		const map = new Map<string, RecordingFile[]>();

		for (const f of files.value) {
			const u = usernameFromFile(f);
			if (!map.has(u)) map.set(u, []);
			map.get(u)!.push(f);
		}

		const result: Group[] = [];
		for (const [username, list] of map) {
			const sorted = [...list].sort((a, b) => {
				let av: number, bv: number;
				if (sortKey.value === "started_at") {
					av = new Date(a.started_at).getTime();
					bv = new Date(b.started_at).getTime();
				} else if (sortKey.value === "size_bytes") {
					av = a.size_bytes;
					bv = b.size_bytes;
				} else {
					av = a.video_duration_secs ?? 0;
					bv = b.video_duration_secs ?? 0;
				}
				return sortDir.value === "desc" ? bv - av : av - bv;
			});
			result.push({
				username,
				files: sorted,
				totalSize: list.reduce((s, f) => s + f.size_bytes, 0),
				hasRecording: list.some((f) => f.is_recording),
			});
		}
		result.sort((a, b) => a.username.localeCompare(b.username));
		return result;
	});

	const allSelectableFiles = computed(() =>
		files.value.filter(
			(f) =>
				!f.is_recording &&
				f.status !== "merging_waiting" &&
				f.status !== "merging",
		),
	);
	const selectedCount = computed(() => selected.value.size);

	function getFileChecked(path: string) {
		return selected.value.has(path);
	}

	function setFileChecked(path: string) {
		if (selected.value.has(path)) selected.value.delete(path);
		else selected.value.add(path);
	}

	function getGroupChecked(group: Group): boolean | "indeterminate" {
		const selectable = group.files.filter(
			(f) =>
				!f.is_recording &&
				f.status !== "merging_waiting" &&
				f.status !== "merging",
		);
		if (selectable.length === 0) return false;
		const n = selectable.filter((f) => selected.value.has(f.path)).length;
		if (n === 0) return false;
		if (n === selectable.length) return true;
		return "indeterminate";
	}

	function setGroupChecked(group: Group) {
		const selectable = group.files.filter(
			(f) =>
				!f.is_recording &&
				f.status !== "merging_waiting" &&
				f.status !== "merging",
		);
		const allSel = selectable.every((f) => selected.value.has(f.path));
		if (allSel) selectable.forEach((f) => selected.value.delete(f.path));
		else selectable.forEach((f) => selected.value.add(f.path));
	}

	function getAllChecked(): boolean | "indeterminate" {
		const selectable = allSelectableFiles.value;
		if (selectable.length === 0) return false;
		const n = selectable.filter((f) => selected.value.has(f.path)).length;
		if (n === 0) return false;
		if (n === selectable.length) return true;
		return "indeterminate";
	}

	function setAllChecked() {
		const selectable = allSelectableFiles.value;
		const allSel = selectable.every((f) => selected.value.has(f.path));
		if (allSel) selectable.forEach((f) => selected.value.delete(f.path));
		else selectable.forEach((f) => selected.value.add(f.path));
	}

	function toggleGroup(username: string) {
		if (collapsedGroups.value.has(username))
			collapsedGroups.value.delete(username);
		else collapsedGroups.value.add(username);
	}

	/**
	 * ，。
	 * Load recording file list from backend and rebuild timer state.
	 */
	async function load() {
		loading.value = true;
		try {
			files.value = await call<RecordingFile[]>("list_recordings");
			rebuildElapsed();
			const paths = new Set(files.value.map((f) => f.path));
			for (const p of selected.value) {
				if (!paths.has(p)) selected.value.delete(p);
			}
		} finally {
			loading.value = false;
		}
	}

	function rebuildElapsed() {
		const next: Record<string, number> = {};
		for (const f of files.value) {
			if (f.is_recording) {
				const current = elapsed.value[f.path] ?? 0;
				next[f.path] = Math.max(current, f.record_duration_secs ?? 0);
			}
		}
		elapsed.value = next;
	}

	function startTick() {
		if (tickTimer) return;
		tickTimer = setInterval(() => {
			for (const path of Object.keys(elapsed.value)) elapsed.value[path]++;
		}, 1000);
	}

	function stopTick() {
		if (tickTimer) {
			clearInterval(tickTimer);
			tickTimer = null;
		}
	}

	function scheduleDirRefresh(afterLoad?: () => void) {
		if (dirRefreshTimer) clearTimeout(dirRefreshTimer);
		dirRefreshTimer = setTimeout(async () => {
			dirRefreshTimer = null;
			await load();
			if (files.value.some((f) => f.is_recording)) startTick();
			else stopTick();
			afterLoad?.();
		}, 300);
	}

	function cleanup() {
		stopTick();
		if (dirRefreshTimer) {
			clearTimeout(dirRefreshTimer);
			dirRefreshTimer = null;
		}
	}

	return {
		files,
		loading,
		elapsed,
		selected,
		selectedCount,
		collapsedGroups,
		groups,
		load,
		rebuildElapsed,
		startTick,
		stopTick,
		scheduleDirRefresh,
		cleanup,
		toggleSort,
		sortIcon,
		toggleGroup,
		getFileChecked,
		setFileChecked,
		getGroupChecked,
		setGroupChecked,
		getAllChecked,
		setAllChecked,
	};
}
