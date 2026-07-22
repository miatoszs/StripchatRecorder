/**
 * Module Translation State Management Store
 *
 * /api/locale/{code} 。
 * postprocess store （ --describe  i18n ）、
 * ， locale/modules/<id>/{code}.json 。
 *
 * Stores module translation override data loaded from the server's /api/locale/{code} endpoint.
 * The postprocess store uses this data (instead of the --describe i18n field) to translate
 * module names, descriptions, and parameter labels, allowing users to customize translations
 * in locale/modules/<id>/{code}.json.
 */

import { defineStore } from "pinia";
import { ref } from "vue";

/*Translation data for a single module */
export interface ModuleLocaleData {
	/*Translated module name */
	name?: string;
	/*Translated module description */
	description?: string;
	/** （key -> {label}）/ Parameter translations (key -> {label}) */
	params?: Record<string, { label?: string }>;
}

export const useModuleLocaleStore = defineStore("moduleLocale", () => {
	/**
	 * ：moduleId ->
	 * Module translation map for the current locale: moduleId -> translation data
	 */
	const locales = ref<Record<string, ModuleLocaleData>>({});

	/*Currently loaded locale code */
	const currentLocale = ref<string>("");

	/**
	 * （ App.vue  locale JSON ）。
	 * Set module translation data for the given locale (called by App.vue after loading locale JSON).
	 *
	 * BCP 47 language tag
	 * Module translation data map
	 */
	function setLocales(
		localeCode: string,
		data: Record<string, unknown>,
	) {
		currentLocale.value = localeCode;
		locales.value = data as Record<string, ModuleLocaleData>;
	}

	/**
	 * 。
	 * Get the translation data for a specific module in the current locale.
	 *
	 * Module unique ID
	 * @returns ， undefined
	 *          Translation data, or undefined if not found
	 */
	function getModuleLocale(moduleId: string): ModuleLocaleData | undefined {
		return locales.value[moduleId];
	}

	return {
		locales,
		currentLocale,
		setLocales,
		getModuleLocale,
	};
});
