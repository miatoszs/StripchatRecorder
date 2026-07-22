/**
 * Post-processing Pipeline State Management Store
 *
 * 。，。
 * （600ms），。
 *
 * Manages the post-processing module list and pipeline configuration.
 * The pipeline consists of ordered nodes, each corresponding to a processing module.
 * Pipeline changes are auto-saved with debounce (600ms) and support real-time multi-client sync.
 */

import { defineStore } from "pinia";
import { ref, watch } from "vue";
import { call, on } from "@/lib/api";
import { useI18n } from "vue-i18n";
import { useModuleLocaleStore } from "@/stores/moduleLocale";

/**
 * ID， crypto.randomUUID()，
 * （ IP  HTTP ） Math.random() 。
 *
 * Generate a random ID, preferring crypto.randomUUID().
 * Falls back to a Math.random()-based implementation in non-secure contexts
 * (e.g. HTTP pages accessed via IP address).
 */
function generateId(): string {
	if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
		return crypto.randomUUID();
	}
	return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
		const r = (Math.random() * 16) | 0;
		return (c === "x" ? r : (r & 0x3) | 0x8).toString(16);
	});
}

/*Module parameter definition */
export interface ParamDef {
	/*Parameter key */
	key: string;
	/*Parameter display label */
	label: string;
	/*Parameter type */
	type: "string" | "number" | "boolean" | "select";
	/*Parameter default value */
	default: unknown;
	/*Options for select type */
	options?: string[];
}

/**
 * JS 。
 * Coerce a parameter default value to the corresponding JS type.
 *
 * Parameter type
 * Raw value
 */
function coerceDefault(
	type: ParamDef["type"],
	value: unknown,
): string | number | boolean {
	if (type === "boolean") return Boolean(value);
	if (type === "number") {
		const n = Number(value);
		return isNaN(n) ? 0 : n;
	}
	if (value === null || value === undefined) return "";
	return String(value);
}

/**  i18n （）/ Module i18n translation for a single locale */
export interface ModuleI18nLocale {
	name?: string;
	description?: string;
	params?: Record<string, { label?: string }>;
}

/*Post-processing module information */
export interface ModuleInfo {
	/*Module unique ID */
	id: string;
	/*Module display name */
	name: string;
	/*Module description */
	description: string;
	/*Module parameter definitions */
	params: ParamDef[];
	/** （）/ i18n translations (optional) */
	i18n?: Record<string, ModuleI18nLocale>;
}

/** （）/ Pipeline node (module instance) */
export interface PipelineNode {
	/**  ID（UUID）/ Node unique ID (UUID) */
	nodeId: string;
	/*Corresponding module ID */
	moduleId: string;
	/*Node parameter values */
	params: Record<string, string | number | boolean>;
	/*Whether this node is enabled */
	enabled: boolean;
}

/*Pipeline configuration */
export interface PipelineConfig {
	nodes: PipelineNode[];
}

export const usePostprocessStore = defineStore("postprocess", () => {
	/*Available post-processing modules */
	const modules = ref<ModuleInfo[]>([]);
	/*Current pipeline configuration */
	const pipeline = ref<PipelineConfig>({ nodes: [] });
	/*Whether loading */
	const loading = ref(false);
	/*Whether saving */
	const saving = ref(false);
	/** （ pipeline-updated ）/ Whether saving locally (to filter self-triggered pipeline-updated events) */
	let _isSavingLocally = false;
	/** （）/ Whether pipeline has been loaded from backend (prevents auto-save before init) */
	let _loaded = false;
	/*Debounce save timer */
	let _saveTimer: ReturnType<typeof setTimeout> | null = null;

	const { locale } = useI18n();
	const moduleLocaleStore = useModuleLocaleStore();

	/**
	 * name/description/params[].label  i18n 。
	 * locale JSON（moduleLocaleStore）， --describe  i18n 。
	 *
	 * Apply i18n translations to module name/description/params[].label based on current locale.
	 * Prefers server-side locale JSON (moduleLocaleStore), falls back to --describe i18n field.
	 */
	function applyModuleI18n(raw: ModuleInfo[]): ModuleInfo[] {
		const lang = locale.value;
		return raw.map((mod) => {
			// Prefer server-side locale JSON
			const serverTr = moduleLocaleStore.getModuleLocale(mod.id);
			// Fall back to --describe i18n field
			const describeTr = mod.i18n?.[lang] as
				| { name?: string; description?: string; params?: Record<string, { label?: string }> }
				| undefined;

			// ：，--describe
			// Merge: server-side takes priority, --describe fills the gaps
			const name =
				serverTr?.name ?? describeTr?.name ?? mod.name;
			const description =
				serverTr?.description ?? describeTr?.description ?? mod.description;
			const params = mod.params.map((p) => ({
				...p,
				label:
					serverTr?.params?.[p.key]?.label ??
					describeTr?.params?.[p.key]?.label ??
					p.label,
			}));

			if (!serverTr && !describeTr) return mod;
			return { ...mod, name, description, params };
		});
	}

	/** （ i18n，）/ Raw module list (before i18n, for re-translating on locale change) */
	const _rawModules = ref<ModuleInfo[]>([]);

	/**
	 * 。
	 * Fetch the available module list from the backend.
	 */
	async function fetchModules() {
		const raw = await call<ModuleInfo[]>("list_modules");
		_rawModules.value = raw;
		modules.value = applyModuleI18n(raw);
	}

	// Re-apply module translations on locale change
	watch([locale, () => moduleLocaleStore.locales], () => {
		if (_rawModules.value.length > 0) {
			modules.value = applyModuleI18n(_rawModules.value);
		}
	});

	/**
	 * 。
	 * Fetch the current pipeline configuration from the backend.
	 */
	async function fetchPipeline() {
		loading.value = true;
		try {
			pipeline.value = await call<PipelineConfig>("get_pipeline");
		} finally {
			loading.value = false;
			_loaded = true;
		}
	}

	/**
	 * 。
	 * Save the current pipeline configuration to the backend.
	 */
	async function savePipeline() {
		saving.value = true;
		_isSavingLocally = true;
		try {
			await call("save_pipeline", { pipeline: pipeline.value });
		} finally {
			saving.value = false;
			setTimeout(() => {
				_isSavingLocally = false;
			}, 500);
		}
	}

	// ， 600ms
	// Watch pipeline changes and auto-save after 600ms debounce
	watch(
		pipeline,
		() => {
			if (!_loaded) return;
			if (_saveTimer) clearTimeout(_saveTimer);
			_saveTimer = setTimeout(() => savePipeline(), 600);
		},
		{ deep: true },
	);

	/**
	 * ，。
	 * Add a new node to the end of the pipeline with the module's default parameter values.
	 *
	 * Module ID to add
	 */
	function addNode(moduleId: string) {
		const mod = modules.value.find((m) => m.id === moduleId);
		if (!mod) return;
		// Initialize params with module-defined defaults
		const defaults: Record<string, string | number | boolean> = {};
		for (const p of mod.params) {
			defaults[p.key] = coerceDefault(p.type, p.default);
		}
		pipeline.value.nodes.push({
			nodeId: generateId(),
			moduleId,
			params: defaults,
			enabled: true,
		});
	}

	/**
	 * 。
	 * Remove a specific node from the pipeline.
	 *
	 * Node ID to remove
	 */
	function removeNode(nodeId: string) {
		pipeline.value.nodes = pipeline.value.nodes.filter(
			(n) => n.nodeId !== nodeId,
		);
	}

	/**
	 * 。
	 * Move a specific node up or down in the pipeline.
	 *
	 * Node ID to move
	 * Move direction
	 */
	function moveNode(nodeId: string, direction: "up" | "down") {
		const idx = pipeline.value.nodes.findIndex((n) => n.nodeId === nodeId);
		if (idx < 0) return;
		const target = direction === "up" ? idx - 1 : idx + 1;
		if (target < 0 || target >= pipeline.value.nodes.length) return;
		const nodes = [...pipeline.value.nodes];
		[nodes[idx], nodes[target]] = [nodes[target], nodes[idx]];
		pipeline.value.nodes = nodes;
	}

	let _moduleWatcherReady = false;
	let _onPipelineUpdated: (() => void) | null = null;

	/**
	 * （）。
	 * Initialize real-time update listeners for modules and pipeline (executed only once).
	 *
	 * Callback when pipeline is updated by another client
	 */
	async function initModuleWatcher(onPipelineUpdated?: () => void) {
		_onPipelineUpdated = onPipelineUpdated ?? null;
		if (_moduleWatcherReady) return;
		_moduleWatcherReady = true;
		await on("modules-changed", () => {
			void fetchModules();
		});
		await on("pipeline-updated", (payload) => {
			// Ignore self-triggered events during local save
			if (_isSavingLocally) return;
			// ，
			// Temporarily disable auto-save to prevent received config from being immediately re-saved
			_loaded = false;
			pipeline.value = payload as PipelineConfig;
			setTimeout(() => {
				_loaded = true;
			}, 0);
			_onPipelineUpdated?.();
		});
	}

	return {
		modules,
		pipeline,
		loading,
		saving,
		fetchModules,
		fetchPipeline,
		savePipeline,
		addNode,
		removeNode,
		moveNode,
		initModuleWatcher,
	};
});
