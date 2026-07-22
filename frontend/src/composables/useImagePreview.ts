/**
 * Image Preview Composable
 *
 * 。
 * （）、，。
 *
 * Provides image preview dialog logic with zoom and pan support.
 * Supports mouse wheel zoom (anchored at cursor), mouse drag panning,
 * and automatically clamps translation to prevent the image from leaving the viewport.
 */

import { ref } from "vue";

/**
 * 。
 * Image preview state and interaction logic.
 */
export function useImagePreview() {
	/*Whether the preview dialog is open */
	const previewOpen = ref(false);
	/*Current preview image URL */
	const previewUrl = ref("");
	/*Current preview image title */
	const previewTitle = ref("");
	/** （1 = ）/ Current zoom scale (1 = fit size) */
	const previewScale = ref(1);
	/** （）/ Current translation offset (pixels) */
	const previewTranslate = ref({ x: 0, y: 0 });
	/*Viewport container element ref */
	const previewViewportRef = ref<HTMLElement | null>(null);
	/*Image element ref */
	const previewImageRef = ref<HTMLImageElement | null>(null);
	/*Whether currently dragging */
	const isDragging = ref(false);

	/**
	 * （ w-fit/h-fit ）。
	 * ， 90vw × (90vh - header) 。
	 *
	 * Computed viewport size that adapts to the image's natural aspect ratio,
	 * fitting within 90vw × (90vh - header). The dialog uses w-fit/h-fit to follow.
	 */
	const viewportSize = ref({ width: "min(90vw, 90vh)", height: "min(90vh, 90vw)" });

	// ：
	// Drag start state: mouse position and translation offset snapshot
	let dragStart = { x: 0, y: 0, tx: 0, ty: 0 };

	/**
	 * [min, max] 。
	 * Clamp a value to the [min, max] range.
	 */
	function clamp(v: number, min: number, max: number) {
		return Math.min(max, Math.max(min, v));
	}

	/**
	 * 。
	 * Reset zoom and translation to initial state.
	 */
	function resetPreviewTransform() {
		previewScale.value = 1;
		previewTranslate.value = { x: 0, y: 0 };
	}

	/**
	 * ，。
	 * Get viewport and image dimension metrics for calculating zoom bounds.
	 *
	 * @returns ， null（）/ Metrics object, or null if elements not ready
	 */
	function getPreviewMetrics() {
		const viewportEl = previewViewportRef.value;
		const imageEl = previewImageRef.value;
		if (!viewportEl || !imageEl) return null;
		if (imageEl.naturalWidth <= 0 || imageEl.naturalHeight <= 0) return null;
		const viewportRect = viewportEl.getBoundingClientRect();
		const viewportWidth = viewportRect.width;
		const viewportHeight = viewportRect.height;
		if (viewportWidth <= 0 || viewportHeight <= 0) return null;
		// Calculate base scale to fit image in viewport
		const fit = Math.min(
			1,
			viewportWidth / imageEl.naturalWidth,
			viewportHeight / imageEl.naturalHeight,
		);
		return {
			viewportRect,
			viewportWidth,
			viewportHeight,
			baseWidth: imageEl.naturalWidth * fit,
			baseHeight: imageEl.naturalHeight * fit,
		};
	}

	/**
	 * ，。
	 * Clamp translation offset to valid range, preventing the image from leaving the viewport.
	 *
	 * Target X offset
	 * Target Y offset
	 * Current scale
	 * Optional pre-computed metrics
	 */
	function clampPreviewTranslate(
		x: number,
		y: number,
		scale: number,
		metrics?: ReturnType<typeof getPreviewMetrics>,
	) {
		// No panning allowed when scale <= 1
		if (scale <= 1) return { x: 0, y: 0 };
		const m = metrics ?? getPreviewMetrics();
		if (!m) return { x, y };
		const maxX = Math.max(0, (m.baseWidth * scale - m.viewportWidth) / 2);
		const maxY = Math.max(0, (m.baseHeight * scale - m.viewportHeight) / 2);
		return { x: clamp(x, -maxX, maxX), y: clamp(y, -maxY, maxY) };
	}

	/**
	 * 。
	 * Reset transform and compute adaptive viewport size when image finishes loading.
	 */
	function onPreviewImageLoad() {
		resetPreviewTransform();

		const img = previewImageRef.value;
		if (!img || img.naturalWidth <= 0 || img.naturalHeight <= 0) return;

		// header  52px（px-4 pt-4 pb-2 + DialogTitle ）
		// Approximate header height: 52px (px-4 pt-4 pb-2 + DialogTitle line height)
		const HEADER_H = 52;
		const maxW = window.innerWidth * 0.9;
		const maxH = window.innerHeight * 0.9 - HEADER_H;
		const ratio = img.naturalWidth / img.naturalHeight;

		// ，
		// Try fitting by width first, then clamp by height
		let w = Math.min(img.naturalWidth, maxW);
		let h = w / ratio;
		if (h > maxH) {
			h = maxH;
			w = h * ratio;
		}

		viewportSize.value = {
			width: `${Math.round(w)}px`,
			height: `${Math.round(h)}px`,
		};
	}

	/**
	 * ，。
	 * Handle mouse wheel zoom, anchored at the cursor position.
	 *
	 * Wheel event
	 */
	function onPreviewWheel(e: WheelEvent) {
		e.preventDefault();
		const metrics = getPreviewMetrics();
		if (!metrics) return;
		const prevScale = previewScale.value;
		const delta = e.deltaY > 0 ? -0.1 : 0.1;
		// Clamp scale to [1, 10]
		const nextScale = Math.min(
			10,
			Math.max(1, Math.round((prevScale + delta) * 100) / 100),
		);
		if (nextScale === prevScale) return;

		// ，
		// Calculate new translation offset anchored at cursor to keep content under cursor stable
		const cursorX = e.clientX - metrics.viewportRect.left;
		const cursorY = e.clientY - metrics.viewportRect.top;
		const curCenterX = metrics.viewportWidth / 2 + previewTranslate.value.x;
		const curCenterY = metrics.viewportHeight / 2 + previewTranslate.value.y;
		const halfW = (metrics.baseWidth * prevScale) / 2;
		const halfH = (metrics.baseHeight * prevScale) / 2;
		const anchorX = clamp(cursorX, curCenterX - halfW, curCenterX + halfW);
		const anchorY = clamp(cursorY, curCenterY - halfH, curCenterY + halfH);
		const localX = (anchorX - curCenterX) / prevScale;
		const localY = (anchorY - curCenterY) / prevScale;
		let nextX = anchorX - metrics.viewportWidth / 2 - localX * nextScale;
		let nextY = anchorY - metrics.viewportHeight / 2 - localY * nextScale;
		({ x: nextX, y: nextY } = clampPreviewTranslate(
			nextX,
			nextY,
			nextScale,
			metrics,
		));
		previewScale.value = nextScale;
		previewTranslate.value = { x: nextX, y: nextY };
	}

	/**
	 * ，（ > 1 ）。
	 * Handle mouse down to start dragging (only when scale > 1).
	 *
	 * Mouse event
	 */
	function onPreviewMousedown(e: MouseEvent) {
		if (e.button !== 0 || previewScale.value <= 1) return;
		isDragging.value = true;
		dragStart = {
			x: e.clientX,
			y: e.clientY,
			tx: previewTranslate.value.x,
			ty: previewTranslate.value.y,
		};
		e.preventDefault();
	}

	/**
	 * ，。
	 * Handle document-level mouse move to update drag translation.
	 *
	 * Mouse event
	 */
	function onDocMousemove(e: MouseEvent) {
		if (!isDragging.value) return;
		previewTranslate.value = clampPreviewTranslate(
			dragStart.tx + (e.clientX - dragStart.x),
			dragStart.ty + (e.clientY - dragStart.y),
			previewScale.value,
		);
	}

	/**
	 * ，。
	 * Handle document-level mouse up to end dragging.
	 */
	function onDocMouseup() {
		isDragging.value = false;
	}

	/**
	 * 。
	 * Open the image preview dialog.
	 *
	 * Image URL
	 * Image title
	 */
	function openPreview(url: string, title: string) {
		previewUrl.value = url;
		previewTitle.value = title;
		resetPreviewTransform();
		// ，
		// Use max size as placeholder until image loads and adapts
		viewportSize.value = {
			width: `${Math.round(window.innerWidth * 0.9)}px`,
			height: `${Math.round(window.innerHeight * 0.9 - 52)}px`,
		};
		previewOpen.value = true;
	}

	return {
		previewOpen,
		previewUrl,
		previewTitle,
		previewScale,
		previewTranslate,
		previewViewportRef,
		previewImageRef,
		isDragging,
		viewportSize,
		resetPreviewTransform,
		onPreviewImageLoad,
		onPreviewWheel,
		onPreviewMousedown,
		onDocMousemove,
		onDocMouseup,
		openPreview,
	};
}
