/**
 * API Communication Layer
 *
 * HTTP REST + SSE  Web 。
 * HTTP REST + SSE real-time event communication layer.
 */

type EventCallback = (payload: unknown) => void;

/*SSE event listener map: event name -> set of callbacks */
const sseListeners = new Map<string, Set<EventCallback>>();
/*Whether SSE is connected */
let sseConnected = false;
/*Resolve function called when SSE connection is ready */
let sseReadyResolve: (() => void) | null = null;

/*Promise that resolves when SSE connection is ready */
const sseReady: Promise<void> = new Promise((resolve) => {
	sseReadyResolve = resolve;
});

/*Callback set for SSE reconnect events */
let sseReconnectCallbacks: Set<() => void> = new Set();
/*Callback set for SSE disconnect events */
let sseDisconnectCallbacks: Set<() => void> = new Set();

/**
 * SSE ，。
 * Register an SSE reconnect callback, returns an unregister function.
 */
export function onSseReconnect(cb: () => void): () => void {
	sseReconnectCallbacks.add(cb);
	return () => sseReconnectCallbacks.delete(cb);
}

/**
 * SSE ，。
 * Register an SSE disconnect callback, returns an unregister function.
 */
export function onSseDisconnect(cb: () => void): () => void {
	sseDisconnectCallbacks.add(cb);
	return () => sseDisconnectCallbacks.delete(cb);
}

/**
 * SSE 。
 * 3 ，。
 *
 * Ensures the SSE connection is established.
 * Auto-reconnects every 3 seconds on disconnect and triggers corresponding callbacks.
 */
function ensureSse() {
	if (sseConnected) return;
	sseConnected = true;
	let isFirstConnect = true;
	let isDisconnected = false;

	const connect = () => {
		const es = new EventSource("/api/events");

		es.onopen = () => {
			sseReadyResolve?.();
			if (!isFirstConnect) {
				sseReconnectCallbacks.forEach((cb) => cb());
			}
			isFirstConnect = false;
			isDisconnected = false;
		};

		es.onmessage = (e) => {
			try {
				const { event, payload } = JSON.parse(e.data) as {
					event: string;
					payload: unknown;
				};
				sseListeners.get(event)?.forEach((cb) => cb(payload));
			} catch {}
		};

		es.onerror = () => {
			es.close();
			if (!isFirstConnect && !isDisconnected) {
				isDisconnected = true;
				sseDisconnectCallbacks.forEach((cb) => cb());
			}
			setTimeout(connect, 3000);
		};
	};

	connect();
}

/**
 * （HTTP REST）。
 * Invoke a backend command via HTTP REST.
 *
 * Command name
 * Command arguments
 * Command return value
 */
export async function call<T = unknown>(
	command: string,
	args?: Record<string, unknown>,
): Promise<T> {
	return httpCall<T>(command, args);
}

/**
 * （SSE）。
 * Subscribe to a backend event via SSE.
 *
 * Event name
 * Event callback function
 * Unsubscribe function
 */
export async function on(
	event: string,
	cb: EventCallback,
): Promise<() => void> {
	ensureSse();
	await sseReady;
	if (!sseListeners.has(event)) sseListeners.set(event, new Set());
	sseListeners.get(event)!.add(cb);
	return () => sseListeners.get(event)?.delete(cb);
}

/**
 * HTTP ： HTTP 、URL 。
 * HTTP command routing table: maps command names to HTTP method, URL, and body builder.
 */
const COMMAND_MAP: Record<
	string,
	{
		method: string;
		url: (args: Record<string, unknown>) => string;
		body?: (args: Record<string, unknown>) => unknown;
	}
> = {
	list_streamers: { method: "GET", url: () => "/api/streamers" },
	add_streamer: {
		method: "POST",
		url: () => "/api/streamers",
		body: (a) => ({ username: a.username }),
	},
	remove_streamer: {
		method: "DELETE",
		url: (a) => `/api/streamers/${a.username}`,
	},
	set_auto_record: {
		method: "POST",
		url: (a) => `/api/streamers/${a.username}/auto-record`,
		body: (a) => ({ enabled: a.enabled }),
	},
	start_recording: {
		method: "POST",
		url: (a) => `/api/streamers/${a.username}/start`,
	},
	stop_recording: {
		method: "POST",
		url: (a) => `/api/streamers/${a.username}/stop`,
	},
	verify_streamer: {
		method: "GET",
		url: (a) => `/api/streamers/${a.username}/verify`,
	},
	get_settings: { method: "GET", url: () => "/api/settings" },
	save_settings_cmd: {
		method: "POST",
		url: () => "/api/settings",
		body: (a) => a.newSettings,
	},
	list_mouflon_keys: { method: "GET", url: () => "/api/mouflon-keys" },
	add_mouflon_key: {
		method: "POST",
		url: () => "/api/mouflon-keys",
		body: (a) => ({ pkey: a.pkey, pdkey: a.pdkey }),
	},
	remove_mouflon_key: {
		method: "DELETE",
		url: (a) => `/api/mouflon-keys/${a.pkey}`,
	},
	sync_mouflon_keys: {
		method: "POST",
		url: () => "/api/mouflon-keys/sync",
	},
	remove_missing_pp_results: {
		method: "POST",
		url: () => "/api/startup-warnings/pp-results",
		body: (a) => ({ paths: a.paths }),
	},
	get_disk_space: { method: "GET", url: () => "/api/disk-space" },
	list_recordings: { method: "GET", url: () => "/api/recordings" },
	get_merging_dirs: { method: "GET", url: () => "/api/recordings/merging" },
	delete_recording: {
		method: "POST",
		url: () => "/api/recordings/delete",
		body: (a) => ({ path: a.path }),
	},
	run_postprocess_cmd: {
		method: "POST",
		url: () => "/api/recordings/postprocess",
		body: (a) => ({ path: a.path }),
	},
	cancel_postprocess: {
		method: "POST",
		url: () => "/api/recordings/postprocess-cancel",
		body: (a) => ({ path: a.path }),
	},
	open_recording: {
		method: "POST",
		url: () => "/api/recordings/open",
		body: (a) => ({ path: a.path }),
	},
	open_output_dir: { method: "POST", url: () => "/api/recordings/open-dir" },
	get_pipeline: { method: "GET", url: () => "/api/pipeline" },
	save_pipeline: {
		method: "POST",
		url: () => "/api/pipeline",
		body: (a) => a.pipeline,
	},
	list_modules: { method: "GET", url: () => "/api/modules" },
	get_postprocess_tasks: { method: "GET", url: () => "/api/postprocess-tasks" },
	get_module_outputs: {
		method: "POST",
		url: () => "/api/recordings/module-outputs",
		body: (a) => ({ path: a.path }),
	},
	list_relay_sessions: {
		method: "GET",
		url: () => "/api/relay/sessions",
	},
	stop_relay: {
		method: "POST",
		url: (a) => `/api/relay/${a.username}/stop`,
	},
};

/**
 * HTTP 。
 * Execute a command via HTTP.
 *
 * Command name, must be defined in COMMAND_MAP
 * Command arguments
 * Parsed response data
 * Throws on unknown command or HTTP failure
 */
async function httpCall<T>(
	command: string,
	args: Record<string, unknown> = {},
): Promise<T> {
	const def = COMMAND_MAP[command];
	if (!def) throw new Error(`Unknown command: ${command}`);

	const url = def.url(args);
	const hasBody = def.body !== undefined;
	const res = await fetch(url, {
		method: def.method,
		headers: hasBody ? { "Content-Type": "application/json" } : undefined,
		body: hasBody ? JSON.stringify(def.body!(args)) : undefined,
	});

	if (!res.ok) {
		const text = await res.text().catch(() => res.statusText);
		throw new Error(text);
	}

	const text = await res.text();
	if (!text) return undefined as T;
	return JSON.parse(text) as T;
}
