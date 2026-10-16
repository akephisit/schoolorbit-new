import { error } from '@sveltejs/kit';
import { PUBLIC_API_URL } from '$app/env/public';
import { readPublicSchools } from '#lib/api/public-schools.js';
import { validSchoolSubdomain } from '#lib/public-site.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, url, setHeaders }) => {
	const after = url.searchParams.get('after');
	setHeaders({ 'cache-control': 'no-store' });
	if (after !== null && !validSchoolSubdomain(after))
		error(400, 'ลิงก์หน้ารายชื่อโรงเรียนไม่ถูกต้อง');
	try {
		return { directory: await readPublicSchools(PUBLIC_API_URL, after, fetch) };
	} catch {
		setHeaders({ 'retry-after': '60' });
		error(503, 'ยังโหลดรายชื่อโรงเรียนไม่ได้ กรุณาลองใหม่อีกครั้ง');
	}
};
