import { json } from '@sveltejs/kit';
import { PUBLIC_BACKEND_URL } from '$env/static/public';
import type { RequestHandler } from './$types';

/**
 * Server-side proxy for portal document upload.
 * ใช้ proxy นี้เพื่อหลีกเลี่ยง CORS — browser เรียก same-origin แทนที่จะเรียก
 * school-api.schoolorbit.app โดยตรง แล้ว SvelteKit server forward ต่อไปที่ backend
 */
export const POST: RequestHandler = async ({ request }) => {
	const formData = await request.formData();

	const backendUrl = `${PUBLIC_BACKEND_URL}/api/admission/portal/upload`;

	// Forward Origin เพื่อให้ backend extract subdomain ได้
	const origin = request.headers.get('origin') ?? '';
	const referer = request.headers.get('referer') ?? '';

	const response = await fetch(backendUrl, {
		method: 'POST',
		body: formData,
		headers: {
			...(origin && { origin }),
			...(referer && { referer })
		}
	});

	const data = await response.json().catch(() => ({ success: false, error: 'Upload failed' }));
	return json(data, { status: response.status });
};
