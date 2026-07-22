/**
 * Notification and Confirm Dialog Composable
 *
 * Toast 。
 * Toast  vue-sonner ；。
 *
 * Provides global Toast message notifications and modal confirmation dialogs.
 * Toast notifications are powered by vue-sonner; confirm dialogs use shared reactive
 * state in a singleton pattern.
 */

import { ref, markRaw } from "vue";
import { toast as sonnerToast } from "vue-sonner";

/*Toast message type */
export type ToastType = "success" | "error" | "info" | "warning";

/*Confirm dialog configuration options */
export interface DialogOptions {
	title: string;
	message: string;
	/*Confirm button text, defaults to "确认" */
	confirmText?: string;
	/*Cancel button text, defaults to "取消" */
	cancelText?: string;
	/** （）/ Whether this is a destructive action (red button) */
	danger?: boolean;
	/*Whether to hide the cancel button */
	hideCancelButton?: boolean;
}

// Promise resolve （）
// Promise resolve function for the current dialog (singleton)
let _dialogResolve: ((confirmed: boolean) => void) | null = null;

// ，null
// Current dialog config, null means no dialog is shown
const dialog = ref<DialogOptions | null>(null);

/**
 * Toast 。
 * Show a Toast notification message.
 *
 * Message content
 * Message type, defaults to "info"
 */
function toast(message: string, type: ToastType = "info") {
	switch (type) {
		case "success":
			sonnerToast.success(message);
			break;
		case "error":
			sonnerToast.error(message);
			break;
		case "warning":
			sonnerToast.warning(message);
			break;
		default:
			sonnerToast.info(message);
	}
}

/**
 * ， Promise。
 * Show a modal confirmation dialog, returns a Promise of whether the user confirmed.
 *
 * Dialog configuration
 * true if confirmed, false if cancelled
 */
function confirm(options: DialogOptions): Promise<boolean> {
	// markRaw  Vue  options
	// Use markRaw to prevent Vue from deeply proxying the options object
	dialog.value = markRaw(options) as DialogOptions;
	return new Promise((resolve) => {
		_dialogResolve = resolve;
	});
}

/**
 * ： Promise 。
 * Internal function: resolves the current dialog's Promise and closes the dialog.
 *
 * @param result - （true=，false=）/ User action result (true=confirm, false=cancel)
 */
function _resolveDialog(result: boolean) {
	dialog.value = null;
	_dialogResolve?.(result);
	_dialogResolve = null;
}

/**
 * 。
 * Returns notification-related utility functions and state.
 */
export function useNotify() {
	return { toast, confirm, dialog, _resolveDialog };
}
