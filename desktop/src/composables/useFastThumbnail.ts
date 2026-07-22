/**
 * Fast Thumbnail Loading Composable
 *
 * CDN ，
 * 。
 *
 * Races multiple CDN TLDs in parallel to find the fastest thumbnail source,
 * reducing image load latency on streamer cards.
 */

import { ref, watch, type Ref } from "vue";

/*Supported CDN top-level domains */
const CDN_TLDS = [
	"doppiocdn.com",
	"doppiocdn.org",
	"doppiocdn.live",
	"doppiocdn.net",
];

/**
 * URL  CDN ， URL。
 * Races the given thumbnail URL across multiple CDNs and returns the fastest one.
 *
 * Reactive ref of the original thumbnail URL
 * Reactive ref of the resolved optimal URL
 */
export function useFastThumbnail(thumbnailUrl: Ref<string | null | undefined>) {
	const resolvedUrl = ref<string | null>(null);

	/**
	 * CDN ， URL。
	 * Try all CDN TLDs in parallel, use the first one that loads successfully.
	 *
	 * Original image URL
	 */
	async function race(url: string) {
		// URL  CDN ， URL
		// Check if URL contains a known CDN TLD, otherwise use the original URL directly
		const matchedTld = CDN_TLDS.find((tld) => url.includes(tld));
		if (!matchedTld) {
			resolvedUrl.value = url;
			return;
		}

		// CDN  Promise，
		// Create an image load Promise for each CDN TLD, take the first to succeed
		const winner = await Promise.any(
			CDN_TLDS.map(
				(tld) =>
					new Promise<string>((resolve, reject) => {
						const candidate = url.replace(matchedTld, tld);
						const img = new Image();
						img.onload = () => resolve(candidate);
						img.onerror = () => reject();
						img.src = candidate;
					}),
			),
		).catch(() => url); // 全部失败时回退到原始 URL / Fall back to original URL if all fail

		resolvedUrl.value = winner;
	}

	// Watch thumbnailUrl changes and re-race
	watch(
		thumbnailUrl,
		(url) => {
			resolvedUrl.value = null;
			if (url) race(url);
		},
		{ immediate: true },
	);

	return resolvedUrl;
}
