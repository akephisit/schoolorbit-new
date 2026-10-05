import { BACKEND_URL, publicRequestFromOrigin, type ApiRequestOptions } from '#lib/api/client.js';
import {
	getRequiredPublicSchoolInfo,
	getPublicSchoolStatistics,
	getPublicSchoolOrganization
} from '#lib/api/school.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { getSchoolPublicIndexing } from '#lib/school-public/seo.js';
import type { PageServerLoad } from './$types';

async function readIdentity(options: ApiRequestOptions) {
	const controller = new AbortController();
	let timeout: ReturnType<typeof setTimeout> | undefined;
	try {
		return await Promise.race([
			getRequiredPublicSchoolInfo({ ...options, signal: controller.signal }),
			new Promise<never>((_, reject) => {
				timeout = setTimeout(() => {
					controller.abort();
					reject(new Error('โหลดข้อมูลโรงเรียนไม่สำเร็จ กรุณาลองใหม่'));
				}, 3_000);
			})
		]);
	} finally {
		clearTimeout(timeout);
	}
}

export const load: PageServerLoad = async ({ fetch, url, setHeaders }) => {
	setHeaders({ 'cache-control': 'no-store' });
	const options = publicRequestFromOrigin(fetch, url.origin);
	const identity = captureRouteLoad(readIdentity(options), 'โหลดข้อมูลโรงเรียนไม่สำเร็จ');
	const statistics = captureRouteLoad(
		getPublicSchoolStatistics(options),
		'โหลดสถิติโรงเรียนไม่สำเร็จ'
	);
	const organization = captureRouteLoad(
		getPublicSchoolOrganization(options),
		'โหลดโครงสร้างบริหารไม่สำเร็จ'
	);
	return {
		site: getSchoolPublicIndexing(url, BACKEND_URL),
		identity: await identity,
		statistics,
		organization
	};
};
