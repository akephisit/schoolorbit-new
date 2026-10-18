import { DEFAULT_APPEARANCE, schoolAppearanceCss, type SchoolAppearanceResponse } from './model.ts';

/** Every document revalidates against its own tenant. No process-wide tenant cache. */
export async function appearanceStylesheet(
	read: (signal: AbortSignal) => Promise<SchoolAppearanceResponse>,
	ifNoneMatch: string | null,
	timeoutMs = 3000
): Promise<Response> {
	const controller = new AbortController();
	let timer: ReturnType<typeof setTimeout> | undefined;
	try {
		const value = await Promise.race([
			read(controller.signal),
			new Promise<never>((_, reject) => {
				timer = setTimeout(() => {
					controller.abort();
					reject(new Error('Appearance timeout'));
				}, timeoutMs);
			})
		]);
		const css = schoolAppearanceCss(value);
		// Revision plus canonical data avoids stale defaults and rolled-back database revisions.
		const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(css));
		const etag =
			'"' +
			Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, '0')).join('') +
			'"';
		const headers = {
			'Content-Type': 'text/css; charset=utf-8',
			'Cache-Control': 'private, no-cache, must-revalidate',
			ETag: etag,
			'X-Content-Type-Options': 'nosniff'
		};
		return new Response(ifNoneMatch === etag ? null : css, {
			status: ifNoneMatch === etag ? 304 : 200,
			headers
		});
	} catch {
		return new Response(schoolAppearanceCss({ appearance: DEFAULT_APPEARANCE, revision: 0 }), {
			headers: {
				'Content-Type': 'text/css; charset=utf-8',
				'Cache-Control': 'no-store',
				'X-Content-Type-Options': 'nosniff'
			}
		});
	} finally {
		clearTimeout(timer);
		controller.abort();
	}
}
