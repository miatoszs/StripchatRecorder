/**
 * Application Settings State Management Store
 *
 * ，、、、。
 * ：，。
 *
 * Manages global recorder configuration including output directory, poll interval,
 * proxy settings, concurrency, and merge format.
 * Supports real-time multi-client sync: automatically updates local state when
 * other clients modify settings via events.
 */

import { defineStore } from "pinia";
import { ref } from "vue";
import { call, on } from "@/lib/api";

/*Application settings data structure */
export interface Settings {
	/*Recording output directory */
	output_dir: string;
	/** （）/ Streamer status poll interval (seconds) */
	poll_interval_secs: number;
	/*Whether auto-record is enabled by default */
	auto_record: boolean;
	/*Stripchat API proxy URL */
	api_proxy_url: string | null;
	/*CDN thumbnail proxy URL */
	cdn_proxy_url: string | null;
	/*Stripchat mirror site URL */
	sc_mirror_url: string | null;
	/** （0 = ）/ Max concurrent recordings (0 = unlimited) */
	max_concurrent: number;
	/** （"mp4"  "mkv"）/ Recording segment merge format ("mp4" or "mkv") */
	merge_format: string;
	/**  tmp （GB，0 = ）/ Max tmp dir size in GB (0 = unlimited) */
	max_tmp_dir_gb: number;
	/*UI language */
	language: string;
	/*Mouflon Keys sync URL */
	mouflon_sync_url: string | null;
	/*Mouflon Keys sync auth token */
	mouflon_sync_token: string | null;
	/*Whether the first-launch setup wizard has been completed */
	setup_done: boolean;
}

/** Mouflon （）/ Mouflon key store (with timestamps) */
export interface MouflonKeysStore {
	/*pkey -> pdkey key pairs */
	keys: Record<string, string>;
	/** （RFC 3339）/ Timestamp of last auto-sync (RFC 3339) */
	auto_synced_at: string | null;
	/** （RFC 3339）/ Timestamp of last manual key change (RFC 3339) */
	manual_updated_at: string | null;
}

export const useSettingsStore = defineStore("settings", () => {
	/*Current settings values */
	const settings = ref<Settings>({
		output_dir: "",
		poll_interval_secs: 30,
		auto_record: true,
		api_proxy_url: null,
		cdn_proxy_url: null,
		sc_mirror_url: null,
		max_concurrent: 0,
		merge_format: "mp4",
		max_tmp_dir_gb: 50,
		language: "en-US",
		mouflon_sync_url: null,
		mouflon_sync_token: null,
		setup_done: false,
	});
	/*Whether loading */
	const loading = ref(false);
	/*Flag briefly set to true after successful save */
	const saved = ref(false);
	/** （ settings-updated ）/ Whether saving locally (to filter self-triggered settings-updated events) */
	const isSavingLocally = ref(false);
	/** （）/ Whether event listeners are initialized (prevents duplicate registration) */
	let listenersReady = false;

	/**
	 * 。
	 * Fetch current settings from the backend.
	 */
	async function fetchSettings() {
		loading.value = true;
		try {
			settings.value = await call<Settings>("get_settings");
		} finally {
			loading.value = false;
		}
	}

	/**
	 * ， 2 。
	 * Save settings to the backend and briefly show a saved indicator for 2 seconds.
	 *
	 * Settings object to save
	 */
	async function saveSettings(s: Settings) {
		isSavingLocally.value = true;
		try {
			await call("save_settings_cmd", { newSettings: s });
			settings.value = s;
			saved.value = true;
			setTimeout(() => (saved.value = false), 2000);
		} finally {
			// 500ms ，
			// Clear local saving flag after 500ms to ensure event filter window is sufficient
			setTimeout(() => {
				isSavingLocally.value = false;
			}, 500);
		}
	}

	/**
	 * （）。
	 * Initialize settings update event listener (executed only once).
	 */
	async function initListeners() {
		if (listenersReady) return;
		listenersReady = true;
		await on("settings-updated", (payload) => {
			// Ignore self-triggered events during local save
			if (isSavingLocally.value) return;
			settings.value = payload as Settings;
		});
	}

	return {
		settings,
		loading,
		saved,
		isSavingLocally,
		fetchSettings,
		saveSettings,
		initListeners,
	};
});
