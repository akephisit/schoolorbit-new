/** Installed mode includes iOS, which exposes standalone separately from the media query. */
export function isStandalonePWA(): boolean {
	return (
		typeof window !== 'undefined' &&
		(window.matchMedia('(display-mode: standalone)').matches ||
			(typeof navigator !== 'undefined' &&
				(navigator as Navigator & { standalone?: boolean }).standalone === true))
	);
}
