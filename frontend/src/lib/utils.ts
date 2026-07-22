/**
 * General Utility Functions
 *
 * Tailwind CSS ， clsx  tailwind-merge 。
 * Provides Tailwind CSS class name merging utility, combining clsx and tailwind-merge
 * for intelligent deduplication and merging.
 */

import type { ClassValue } from "clsx";
import { clsx } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * Tailwind CSS ，。
 * Merges Tailwind CSS class names, automatically handling conflicts and duplicates.
 *
 * @param inputs - （、、）
 *                 Any number of class name values (strings, objects, arrays, etc.)
 * Merged class name string
 */
export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

/**
 * ，（ http://0.0.0.0）。
 * Writes text to clipboard with fallback for non-secure contexts (e.g. http://0.0.0.0).
 *
 * Text to copy
 * Whether it succeeded
 */
export async function copyToClipboard(text: string): Promise<boolean> {
	// Clipboard API（）
	// Prefer modern Clipboard API (requires secure context)
	if (navigator.clipboard?.writeText) {
		try {
			await navigator.clipboard.writeText(text);
			return true;
		} catch {
			// Fall through to legacy fallback on permission denial
		}
	}
	// ： textarea + execCommand
	// Legacy fallback: use temporary textarea + execCommand
	try {
		const el = document.createElement("textarea");
		el.value = text;
		el.style.cssText = "position:fixed;top:-9999px;left:-9999px;opacity:0";
		document.body.appendChild(el);
		el.focus();
		el.select();
		const ok = document.execCommand("copy");
		document.body.removeChild(el);
		return ok;
	} catch {
		return false;
	}
}
