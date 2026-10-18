/** DOM adapter for the existing uiPreferences mode; no separate preference or storage. */
export function applyPersonalMode(preference: 'light' | 'dark' | 'system', pathname: string): void {
	const publicPage = pathname === '/' || pathname.startsWith('/login');
	const dark =
		preference === 'dark' ||
		(preference === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
	document.documentElement.classList.toggle('dark', !publicPage && dark);
	document
		.querySelector('meta[name="theme-color"]')
		?.setAttribute(
			'content',
			getComputedStyle(document.documentElement).getPropertyValue('--background').trim()
		);
}
