/**
 * i18n Initialization Module
 *
 * ：
 * 1.  /api/locale/{code} （ <exe_dir>/locale/app/{code}.json），
 * en-US）作为初始消息和 fallback，确保首屏不闪烁
 *
 * /api/locales ，。
 *
 * Locale data loading priority:
 * 1. Load from backend /api/locale/{code}, allowing user customization
 * 2. Built-in TS translations (en-US / en-US) as initial messages and fallback
 *
 * Available locale list is fetched dynamically from /api/locales —
 * adding a new language requires no frontend code changes.
 */

import { createI18n } from "vue-i18n";
import enUS from "./locales/en-US";

export type MessageSchema = typeof enUS;

const savedLocale = "en-US"; // 初始值，启动后由 App.vue 从后端 settings 同步覆盖
const storedLang = localStorage.getItem("lang");
const lang = storedLang || savedLocale;

/** （ /api/locales ）/ Available locale entry (from /api/locales) */
export interface LocaleEntry {
	/*BCP 47 locale code */
	code: string;
	/*Native display name */
	name: string;
}

/**
 * 。
 * Fetch the list of available locales from the server.
 */
export async function fetchAvailableLocales(): Promise<LocaleEntry[]> {
	try {
		const res = await fetch("/api/locales");
		if (!res.ok) return builtinLocales();
		const data = await res.json();
		if (Array.isArray(data) && data.length > 0) return data as LocaleEntry[];
		return builtinLocales();
	} catch {
		return builtinLocales();
	}
}

/**  fallback （）/ Built-in fallback locale list */
function builtinLocales(): LocaleEntry[] {
	return [
		{ code: "en-US", name: "English" },
	];
}

/*Result of loading a locale */
export interface LoadLocaleResult {
	/** （moduleId -> {name, description, params}）/ Module translation map */
	modules: Record<string, unknown>;
	/**
	 * ，； undefined。
	 * Set when the locale file exists but fails validation; otherwise undefined.
	 */
	warning?: string;
}

/**
 * API  locale ，
 * vue-i18n（），。
 * 。
 *
 * Fetch the full locale data from the backend for the given locale code,
 * dynamically register it in vue-i18n if not already registered,
 * and deep-merge to override built-in messages.
 * Returns module translation overrides and any file validation warning.
 *
 * BCP 47 language tag
 * LoadLocaleResult, modules is {} on failure
 */
export async function loadLocaleFromServer(
	localeCode: string,
): Promise<LoadLocaleResult> {
	try {
		const res = await fetch(`/api/locale/${encodeURIComponent(localeCode)}`);
		if (!res.ok) return { modules: {} };
		const data = await res.json();

		if (data.app && typeof data.app === "object") {
			// vue-i18n ，
			// Register with empty object first if locale not yet known, then merge
			if (!(i18n.global.availableLocales as string[]).includes(localeCode)) {
				i18n.global.setLocaleMessage(localeCode as any, data.app);
			} else {
				i18n.global.mergeLocaleMessage(localeCode as any, data.app);
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

export const i18n = createI18n({
	legacy: false, // 必须为 false 才能使用 Composition API
	locale: lang,
	// en-US 。
	// fetch  JSON  locale 。
	// Built-in en-US provide initial messages to prevent first-frame flash.
	// Later backend fetches will merge full JSON into the corresponding locale.
	fallbackLocale: "en-US",
	messages: {
		"en-US": enUS,
	},
});

export default i18n;
