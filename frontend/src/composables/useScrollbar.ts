/**
 * Overlay scrollbar composable
 *
 * scroll ， .is-scrolling class ，
 * 800ms  class， CSS transition 。
 *
 * Listens to scroll events on the target element, adds .is-scrolling class while
 * scrolling to reveal the scrollbar, then removes it 800ms after scrolling stops
 * so the scrollbar fades out via CSS transition.
 *
 * Usage:
 *   const el = ref<HTMLElement | null>(null)
 *   useScrollbar(el)
 *   // <div ref="el" class="scrollbar-overlay overflow-y-scroll">
 */

import { watch, onUnmounted } from "vue";
import type { Ref } from "vue";

export function useScrollbar(elRef: Ref<HTMLElement | null>, delay = 800) {
	let timer: ReturnType<typeof setTimeout> | null = null;
	let cleanup: (() => void) | null = null;

	function onScroll() {
		const el = elRef.value;
		if (!el) return;
		el.classList.add("is-scrolling");
		if (timer !== null) clearTimeout(timer);
		timer = setTimeout(() => {
			el.classList.remove("is-scrolling");
			timer = null;
		}, delay);
	}

	watch(
		elRef,
		(el, prevEl) => {
			// Clean up listener on previous element
			if (prevEl) {
				prevEl.removeEventListener("scroll", onScroll);
			}
			if (el) {
				el.addEventListener("scroll", onScroll, { passive: true });
			}
			cleanup = () => {
				if (el) el.removeEventListener("scroll", onScroll);
				if (timer !== null) clearTimeout(timer);
			};
		},
		{ immediate: true },
	);

	onUnmounted(() => {
		cleanup?.();
	});
}
