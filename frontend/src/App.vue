<!--
    应用根组件 / Application Root Component

    提供侧边栏导航和主内容区域的整体布局。
    负责：
    - 跟随系统主题自动切换深色/浅色模式
    - 监听 ffmpeg-missing 事件并显示警告
    - 监听 SSE 断开/重连事件，重连后自动刷新页面
    - 监听 startup-warnings 事件，处理不存在的主播和孤立的后处理记录

    Provides the overall layout with sidebar navigation and main content area.
    Responsible for:
    - Auto dark/light mode following system theme
    - Listening for ffmpeg-missing events and showing warnings
    - Listening for SSE disconnect/reconnect events, auto-reloading on reconnect
    - Listening for startup-warnings to handle non-existent streamers and orphaned post-processing records
-->
<script setup lang="ts">
	import { onMounted, onUnmounted, ref } from "vue";
	import { RouterView, useRouter, useRoute } from "vue-router";
	import NotifyLayer from "./components/NotifyLayer.vue";
	import { Button } from "@/components/ui/button";
	import { call, on, onSseReconnect, onSseDisconnect } from "@/lib/api";
	import { useNotify } from "@/composables/useNotify";
	import { toast as sonnerToast } from "vue-sonner";
	import { useStreamersStore } from "@/stores/streamers";
	import { useI18n } from "vue-i18n";
	import { useScrollbar } from "@/composables/useScrollbar";
	import { loadLocaleFromServer } from "@/i18n";
	import { useModuleLocaleStore } from "@/stores/moduleLocale";
	import { useLocalesStore } from "@/stores/locales";

	const router = useRouter();
	const route = useRoute();
	const { toast, confirm } = useNotify();
	const streamersStore = useStreamersStore();
	const { t, locale } = useI18n();
	const moduleLocaleStore = useModuleLocaleStore();
	const localesStore = useLocalesStore();

	const mainScrollEl = ref<HTMLElement | null>(null);
	useScrollbar(mainScrollEl);

	/*Sidebar navigation items configuration */
	const navItems = [
		{ to: "/", labelKey: "nav.streamers" },
		{ to: "/recordings", labelKey: "nav.recordings" },
		{ to: "/postprocess", labelKey: "nav.postprocess" },
		{ to: "/relay", labelKey: "nav.relay" },
		{ to: "/finder", labelKey: "nav.finder" },
		{ to: "/settings", labelKey: "nav.settings" },
	];

	/**
	 * dark ，/。
	 * Toggle the dark class on the document root element for dark/light theme switching.
	 *
	 * Whether to apply dark theme
	 */
	function applyTheme(dark: boolean) {
		document.documentElement.classList.toggle("dark", dark);
	}

	// Listen for system theme changes
	const mq = window.matchMedia("(prefers-color-scheme: dark)");
	function onThemeChange(e: MediaQueryListEvent) {
		applyTheme(e.matches);
	}

	// Event unsubscribe functions
	let unlistenFfmpeg: (() => void) | null = null;
	let unlistenReconnect: (() => void) | null = null;
	let unlistenDisconnect: (() => void) | null = null;
	let unlistenWarnings: (() => void) | null = null;
	let unlistenLocaleWarnings: (() => void) | null = null;

	/**
	 * ：
	 * 1.  ->
	 * 2. （）->
	 *
	 * Handle startup warning events:
	 * 1. Non-existent streamer accounts -> prompt user and auto-delete
	 * 2. Orphaned post-processing records (files deleted) -> prompt user and clean up
	 */
	async function handleStartupWarnings(payload: unknown) {
		const w = payload as {
			missing_streamers: string[];
			missing_pp_results: string[];
		};

		if (w.missing_streamers.length > 0) {
			const ok = await confirm({
				title: t("notify.missingStreamers.title"),
				message: t("notify.missingStreamers.message", { list: w.missing_streamers.join("\n") }),
				confirmText: t("notify.missingStreamers.confirm"),
				cancelText: t("notify.missingStreamers.ignore"),
				danger: true,
			});
			if (ok) {
				for (const username of w.missing_streamers) {
					await streamersStore.removeStreamer(username).catch(() => {});
				}
				toast(t("notify.missingStreamers.done", { count: w.missing_streamers.length }), "success");
			}
		}

		if (w.missing_pp_results.length > 0) {
			const ok = await confirm({
				title: t("notify.missingPpResults.title"),
				message: t("notify.missingPpResults.message", { list: w.missing_pp_results.map((p) => p.split(/[\\/]/).pop()).join("\n") }),
				confirmText: t("notify.missingPpResults.confirm"),
				cancelText: t("notify.missingPpResults.ignore"),
			});
			if (ok) {
				await call("remove_missing_pp_results", {
					paths: w.missing_pp_results,
				}).catch(() => {});
				toast(t("notify.missingPpResults.done", { count: w.missing_pp_results.length }), "success");
			}
		}
	}

	onMounted(async () => {
		// Initialize theme and listen for system theme changes
		applyTheme(mq.matches);
		mq.addEventListener("change", onThemeChange);

		// ， locale
		// Sync language from backend, load messages before switching locale
		try {
			const settings = await call<{ language?: string }>("get_settings");
			if (settings?.language) {
				// ， locale，
				// Load messages for the language first, then switch locale,
				// so the first render already uses the correct language
				const { modules: moduleLocales } = await loadLocaleFromServer(settings.language);
				locale.value = settings.language;
				moduleLocaleStore.setLocales(settings.language, moduleLocales);
			} else {
				// ， locale （）
				// No custom language, still load server overrides for the default locale
				const { modules: moduleLocales } = await loadLocaleFromServer(locale.value);
				moduleLocaleStore.setLocales(locale.value, moduleLocales);
			}
		} catch {
			// locale  fallback
			// Backend not ready: load current locale messages as fallback
			const { modules: moduleLocales } = await loadLocaleFromServer(locale.value);
			moduleLocaleStore.setLocales(locale.value, moduleLocales);
		}

		// Listen for ffmpeg missing warning
		unlistenFfmpeg = await on("ffmpeg-missing", (payload) => {
			const p = payload as { message: string };
			toast(p.message, "warning");
		});

		// SSE  3 ，
		// After SSE reconnect, countdown 3 seconds then reload to sync state with server
		unlistenReconnect = onSseReconnect(() => {
			const COUNTDOWN = 3;
			let remaining = COUNTDOWN;
			const id = "reconnect-reload";
			sonnerToast.info(t("notify.reconnected", { n: remaining }), {
				id,
				duration: (COUNTDOWN + 1) * 1000,
			});
			const timer = setInterval(() => {
				remaining--;
				if (remaining > 0) {
					sonnerToast.info(t("notify.reconnected", { n: remaining }), {
						id,
						duration: (remaining + 1) * 1000,
					});
				} else {
					clearInterval(timer);
					window.location.reload();
				}
			}, 1000);
		});

		// Listen for SSE disconnect
		unlistenDisconnect = onSseDisconnect(() => {
			toast(t("notify.disconnected"), "warning");
		});

		// Listen for startup warnings
		unlistenWarnings = await on("startup-warnings", handleStartupWarnings);

		// Listen for custom locale file validation warnings
		unlistenLocaleWarnings = await on(
			"locale-warnings",
			(payload) => {
				const items = payload as Array<{ path: string; reason: string }>;
				for (const item of items) {
					const file = item.path.replace(/\\/g, "/").split("/").pop() ?? item.path;
					toast(`${t("settings.localeFileInvalid", { file })}: ${item.reason}`, "warning");
				}
			},
		);

		// Initial load of available locales
		await localesStore.refresh();

		// locale-files-changed  localesStore ，
		// locale-files-changed is already listened inside localesStore; no need to register here
	});

	onUnmounted(() => {
		// Clean up all event listeners
		mq.removeEventListener("change", onThemeChange);
		unlistenFfmpeg?.();
		unlistenReconnect?.();
		unlistenDisconnect?.();
		unlistenWarnings?.();
		unlistenLocaleWarnings?.();
	});
</script>

<template>
	<!-- 全局布局过渡：setup 页面与主页面之间的切换 / Global layout transition between setup and main -->
	<Transition name="layout" mode="out-in">

		<!-- setup 页面：全屏无侧边栏 / Setup page: full-screen without sidebar -->
		<div v-if="route.path === '/setup'" key="setup" class="contents">
			<RouterView v-slot="{ Component }">
				<Transition name="page" mode="out-in">
					<component :is="Component" :key="route.path" />
				</Transition>
			</RouterView>
			<NotifyLayer />
		</div>

		<!-- 正常布局：侧边栏 + 内容区 / Normal layout: sidebar + content -->
		<div v-else key="main" class="flex h-screen overflow-hidden">
			<aside
				class="w-44 shrink-0 bg-sidebar border-r border-sidebar-border flex flex-col p-3 gap-1"
			>
				<div
					class="flex items-center gap-2 px-1 py-4 mb-1 border-b border-sidebar-border"
				>
					<span class="w-2.5 h-2.5 rounded-full bg-destructive shrink-0" />
					<span class="text-sm font-bold text-sidebar-foreground"
						>StripchatRecorder</span
					>
				</div>
				<nav class="flex flex-col gap-0.5">
					<Button
						v-for="item in navItems"
						:key="item.to"
						variant="ghost"
						class="w-full justify-start text-sm font-normal"
						:class="
							route.path === item.to
								? 'bg-sidebar-accent text-sidebar-accent-foreground font-semibold'
								: 'text-sidebar-foreground/70 hover:text-sidebar-foreground hover:bg-sidebar-accent/50'
						"
						@click="router.push(item.to)"
					>
						{{ t(item.labelKey) }}
					</Button>
				</nav>
			</aside>
			<main class="flex-1 overflow-hidden">
				<div ref="mainScrollEl" class="h-full overflow-y-scroll p-6 scrollbar-overlay">
					<RouterView v-slot="{ Component }">
						<Transition name="page" mode="out-in">
							<component :is="Component" :key="route.path" />
						</Transition>
					</RouterView>
				</div>
			</main>
			<NotifyLayer />
		</div>

	</Transition>
</template>
