/**
 * Available Locales Store
 *
 * ， SetupView、SettingsView 。
 * store  `locale-files-changed` ，。
 *
 * Centrally manages the available locale list fetched from the backend,
 * shared by SetupView and SettingsView.
 * Automatically subscribes to `locale-files-changed` on init to refresh on file changes.
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
	 * 。
	 * Fetch the latest available locale list from the backend.
	 */
	async function refresh() {
		locales.value = await fetchAvailableLocales();
		loaded.value = true;
	}

	// store ， App.vue  onMounted
	// Subscribe to file change events when store is created, independent of App.vue's onMounted timing
	on("locale-files-changed", () => {
		refresh();
	});

	return { locales, loaded, refresh };
});
