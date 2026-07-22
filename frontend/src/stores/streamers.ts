/**
 * Streamer State Management Store
 *
 * ，、、。
 * SSE/Tauri 。
 *
 * Manages the state of all tracked streamers, including online status, recording state,
 * viewer count, and thumbnails. Synchronizes state changes across multiple clients
 * in real-time via SSE/Tauri events.
 */

import { defineStore } from "pinia";
import { ref } from "vue";
import { call, on } from "@/lib/api";
import { toast as sonnerToast } from "vue-sonner";

/*Streamer entry data structure */
export interface StreamerEntry {
	username: string;
	/*Whether auto-record is enabled */
	auto_record: boolean;
	/** （ISO ）/ Time added (ISO string) */
	added_at: string;
	is_online: boolean;
	is_recording: boolean;
	/** （）/ Whether the stream is recordable (publicly accessible) */
	is_recordable: boolean;
	viewers: number;
	/** （""）/ Stream status text (e.g. "") */
	status: string;
	thumbnail_url: string | null;
	/*Whether the stream is being relayed */
	is_relaying?: boolean;
}

/*Status update event payload */
export interface StatusUpdatePayload {
	username: string;
	is_online: boolean;
	is_recording: boolean;
	is_recordable: boolean;
	viewers: number;
	status: string;
	thumbnail_url: string | null;
}

export const useStreamersStore = defineStore("streamers", () => {
	/*Streamer list */
	const streamers = ref<StreamerEntry[]>([]);
	/*Whether loading */
	const loading = ref(false);
	/*Most recent error message */
	const error = ref<string | null>(null);
	/** （）/ Set of usernames with stop-recording in progress (prevents status flicker) */
	const stoppingSet = ref(new Set<string>());
	/** （）/ Local action markers (to filter self-triggered event notifications) */
	const localActions = new Set<string>();
	/** （）/ Whether event listeners are initialized (prevents duplicate registration) */
	let listenersReady = false;

	/**
	 * ， TTL 。
	 * Mark an action as locally initiated; ignore corresponding remote event notifications within TTL.
	 *
	 * @param key - （ "add:username"）/ Action key (e.g. "add:username")
	 * Marker TTL (ms), defaults to 3000ms
	 */
	function markLocal(key: string, ttl = 3000) {
		localActions.add(key);
		setTimeout(() => localActions.delete(key), ttl);
	}

	/**
	 * 。
	 * Fetch the streamer list from the backend.
	 */
	async function fetchStreamers() {
		loading.value = true;
		try {
			streamers.value = await call<StreamerEntry[]>("list_streamers");
		} catch (e) {
			error.value = String(e);
		} finally {
			loading.value = false;
		}
	}

	/**
	 * 。
	 * Add a new streamer to the tracking list.
	 *
	 * Streamer username
	 */
	async function addStreamer(username: string) {
		markLocal(`add:${username}`);
		await call("add_streamer", { username });
		await fetchStreamers();
	}

	/**
	 * 。
	 * Remove a streamer from the tracking list.
	 *
	 * Streamer username
	 */
	async function removeStreamer(username: string) {
		markLocal(`remove:${username}`);
		await call("remove_streamer", { username });
		streamers.value = streamers.value.filter((s) => s.username !== username);
	}

	/**
	 * 。
	 * Set the auto-record toggle for a streamer.
	 *
	 * Streamer username
	 * Whether to enable auto-record
	 */
	async function setAutoRecord(username: string, enabled: boolean) {
		markLocal(`auto:${username}`);
		await call("set_auto_record", { username, enabled });
		const s = streamers.value.find((s) => s.username === username);
		if (s) s.auto_record = enabled;
	}

	/**
	 * 。
	 * Manually start recording a specific streamer.
	 *
	 * Streamer username
	 * Recording file path
	 */
	async function startRecording(username: string): Promise<string> {
		return call<string>("start_recording", { username });
	}

	/**
	 * 。
	 * false， UI 。
	 *
	 * Manually stop recording a specific streamer.
	 * Immediately sets recording state to false locally to prevent UI flicker.
	 *
	 * Streamer username
	 */
	async function stopRecording(username: string) {
		stoppingSet.value.add(username);
		const s = streamers.value.find((s) => s.username === username);
		if (s) s.is_recording = false;
		await call("stop_recording", { username });
	}

	/**
	 * 。
	 * Stop stream relay for the given streamer.
	 *
	 * Streamer username
	 */
	async function stopRelay(username: string) {
		await call("stop_relay", { username });
		const s = streamers.value.find((s) => s.username === username);
		if (s) s.is_relaying = false;
	}

	/**
	 * （）。
	 * /、、/、。
	 *
	 * Initialize backend event listeners (executed only once).
	 * Listens for streamer add/remove, status updates, recording start/stop, auto-record changes, etc.
	 */
	async function initListeners() {
		if (listenersReady) return;
		listenersReady = true;
		await Promise.all([
			on("streamer-added", (payload) => {
				const p = payload as { username: string };
				// Show notification for non-local actions
				if (!localActions.has(`add:${p.username}`)) {
					sonnerToast.info(`其他客户端添加了主播：${p.username}`);
				}
				void fetchStreamers();
			}),
			on("streamer-removed", (payload) => {
				const p = payload as { username: string };
				if (!localActions.has(`remove:${p.username}`)) {
					sonnerToast.info(`其他客户端移除了主播：${p.username}`);
				}
				streamers.value = streamers.value.filter(
					(s) => s.username !== p.username,
				);
			}),
			on("status-update", (payload) => {
				const p = payload as StatusUpdatePayload;
				const s = streamers.value.find((s) => s.username === p.username);
				if (s) {
					// ，，
					// If stop is in progress, ignore backend recording state to prevent flicker
					const isStopping = stoppingSet.value.has(p.username);
					Object.assign(s, {
						is_online: p.is_online,
						is_recording: isStopping ? false : p.is_recording,
						is_recordable: isStopping ? s.is_recordable : p.is_recordable,
						viewers: p.viewers,
						status: p.status,
						// ，
						// Only update thumbnail if a new one is provided
						...(p.thumbnail_url ? { thumbnail_url: p.thumbnail_url } : {}),
					});
				}
			}),
			on("recording-started", (payload) => {
				const p = payload as { username: string; file_path: string };
				stoppingSet.value.delete(p.username);
				const s = streamers.value.find((s) => s.username === p.username);
				if (s) s.is_recording = true;
			}),
			on("recording-stopped", (payload) => {
				const p = payload as { username: string };
				stoppingSet.value.delete(p.username);
				const s = streamers.value.find((s) => s.username === p.username);
				if (s) s.is_recording = false;
			}),
			on("auto-record-changed", (payload) => {
				const p = payload as { username: string; enabled: boolean };
				if (!localActions.has(`auto:${p.username}`)) {
					sonnerToast.info(
						`其他客户端${p.enabled ? "开启" : "关闭"}了 ${p.username} 的自动录制`,
					);
				}
				const s = streamers.value.find((s) => s.username === p.username);
				if (s) s.auto_record = p.enabled;
			}),
			on("api-error", (payload) => {
				const p = payload as { message: string };
				sonnerToast.error(`Stripchat API连接错误: ${p.message}`);
			}),
		]);
	}

	return {
		streamers,
		loading,
		error,
		fetchStreamers,
		addStreamer,
		removeStreamer,
		setAutoRecord,
		startRecording,
		stopRecording,
		stopRelay,
		initListeners,
	};
});
