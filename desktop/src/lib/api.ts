/**
 * API （Tauri ）/ API Communication Layer (Tauri Desktop)
 *
 * HTTP REST + SSE  Tauri IPC：
 * - `call()` → `@tauri-apps/api/core invoke()`
 * - `on()`   → `@tauri-apps/api/event listen()`
 *
 * Replaces HTTP REST + SSE with Tauri IPC:
 * - `call()` → `@tauri-apps/api/core invoke()`
 * - `on()`   → `@tauri-apps/api/event listen()`
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ─── （ Tauri listen）/ Event system (Tauri listen-based) ───────

/**
 * SSE （Tauri ，）。
 * SSE reconnect callbacks (no reconnect needed in Tauri mode, kept for interface compatibility).
 */
let sseReconnectCallbacks: Set<() => void> = new Set();
/**
 * SSE （，Tauri ）。
 * SSE disconnect callbacks (same as above, no such concept in Tauri).
 */
let sseDisconnectCallbacks: Set<() => void> = new Set();

/**
 * SSE （Tauri ）。
 * Register SSE reconnect callback (no-op in Tauri mode).
 */
export function onSseReconnect(cb: () => void): () => void {
	sseReconnectCallbacks.add(cb);
	return () => sseReconnectCallbacks.delete(cb);
}

/**
 * SSE （Tauri ）。
 * Register SSE disconnect callback (no-op in Tauri mode).
 */
export function onSseDisconnect(cb: () => void): () => void {
	sseDisconnectCallbacks.add(cb);
	return () => sseDisconnectCallbacks.delete(cb);
}

/**
 * Tauri 。
 * Invoke a backend Tauri command.
 *
 * （command） #[tauri::command] （snake_case）。
 * Command names map directly to #[tauri::command] function names (snake_case).
 *
 * Tauri command name
 * @param args    - （）/ Command arguments (keys match parameter names)
 */
export async function call<T = unknown>(
	command: string,
	args?: Record<string, unknown>,
): Promise<T> {
	return invoke<T>(command, args);
}

/**
 * Tauri 。
 * Subscribe to a Tauri backend event.
 *
 * server  SSE （ Emitter ）。
 * Event names are identical to server mode SSE event names
 * (backend Emitter emits the same names).
 *
 * Event name
 * Event callback
 * Unsubscribe function
 */
export async function on(
	event: string,
	cb: (payload: unknown) => void,
): Promise<() => void> {
	const unlisten: UnlistenFn = await listen<unknown>(event, (e) => {
		cb(e.payload);
	});
	return unlisten;
}
