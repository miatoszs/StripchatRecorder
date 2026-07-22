/**
 * i18n （）/ i18n Initialization Module (Desktop)
 *
 * ： Tauri invoke  fetch  locale 。
 * Difference from server version: loads locale data via Tauri invoke instead of fetch.
 */

import { createI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import zhCN from "./locales/zh-CN";
import enUS from "./locales/en-US";

export type MessageSchema = typeof zhCN;

const savedLocale = "zh-CN";

/*Available locale entry */
export interface LocaleEntry {
	code: string;
	name: string;
}

/**
 * （ Tauri invoke）。
 * Fetch available locales via Tauri invoke.
 */
export async function fetchAvailableLocales(): Promise<LocaleEntry[]> {
	try {
		const data = await invoke<LocaleEntry[]>("list_locales");
		if (Array.isArray(data) && data.length > 0) return data;
		return builtinLocales();
	} catch {
		return builtinLocales();
	}
}

function builtinLocales(): LocaleEntry[] {
	return [
		{ code: "zh-CN", name: "简体中文" },
		{ code: "en-US", name: "English" },
	];
}

export interface LoadLocaleResult {
	modules: Record<string, unknown>;
	warning?: string;
}

/**
 * locale （ Tauri invoke）。
 * Load the full locale data for the given locale code via Tauri invoke.
 */
export async function loadLocaleFromServer(
	localeCode: string,
): Promise<LoadLocaleResult> {
	try {
		const data = await invoke<Record<string, unknown>>("get_locale", {
			localeCode,
		});

		if (data.app && typeof data.app === "object") {
			if (!i18n.global.availableLocales.includes(localeCode as never)) {
				i18n.global.setLocaleMessage(localeCode as never, data.app as Record<string, unknown>);
			} else {
				i18n.global.mergeLocaleMessage(localeCode as never, data.app as Record<string, unknown>);
			}
		}

		return {
			modules: (data.modules as Record<string, unknown>) ?? {},
			warning: typeof data.warning === "string" ? data.warning : undefined,
		};
	} catch {
		return { modules: {} };
	}
}

const i18n = createI18n<false>({
	legacy: false,
	locale: savedLocale,
	fallbackLocale: "zh-CN",
	messages: {
		"zh-CN": zhCN,
		"en-US": enUS,
	},
});

export default i18n;
