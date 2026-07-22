/**
 * Available Locales Store
 *
 * ， SetupView、SettingsView 。
 * App.vue  onMounted  setup() 。
 *
 * Centrally manages the available locale list fetched from the backend,
 * shared by SetupView and SettingsView.
 * Call setup() in App.vue's onMounted to register event listeners properly.
 */

import { defineStore } from "pinia";
import { ref } from "vue";
import { fetchAvailableLocales, type LocaleEntry } from "@/i18n";
import { on } from "@/lib/api";

export const useLocalesStore = defineStore("locales", () => {
	/*Available locale list */
	const locales = ref<LocaleEntry[]>([]);
	/*Whether the initial load has completed */
	const loaded = ref(false);

	/**
	 * ，。
	 * Fetch the latest available locale list; only update if it changed.
	 */
	async function refresh() {
		const latest = await fetchAvailableLocales();
		const latestJson = JSON.stringify(latest.map((l) => l.code).sort());
		const currentJson = JSON.stringify(locales.value.map((l) => l.code).sort());
		if (latestJson !== currentJson) {
			locales.value = latest;
		}
		loaded.value = true;
	}

	/**
	 * 。 App.vue  onMounted  await ，
	 * Tauri webview （desktop ） SSE （server ）。
	 *
	 * Register event listeners. Must be awaited in App.vue's onMounted
	 * to ensure Tauri webview is ready (desktop) or SSE is connected (server).
	 */
	async function setupListeners() {
		await on("locale-files-changed", () => {
			refresh();
		});
	}

	return { locales, loaded, refresh, setupListeners };
});
