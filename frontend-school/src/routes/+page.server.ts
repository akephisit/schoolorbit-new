import { BACKEND_URL, publicRequestFromOrigin } from '#lib/api/client.js';
import { getPublicSchoolStatistics } from '#lib/api/school.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { getSchoolPublicIndexing } from '#lib/school-public/seo.js';
import { readPublicSchoolIdentity } from '#lib/server/public-school.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, url, setHeaders }) => {
	setHeaders({ 'cache-control': 'no-store' });
	const options = publicRequestFromOrigin(fetch, url.origin);
	const identity = captureRouteLoad(
		readPublicSchoolIdentity(options),
		'โหลดข้อมูลโรงเรียนไม่สำเร็จ'
	);
	const statistics = captureRouteLoad(
		getPublicSchoolStatistics(options),
		'โหลดสถิติโรงเรียนไม่สำเร็จ'
	);
	return {
		site: getSchoolPublicIndexing(url, BACKEND_URL),
		identity: await identity,
		statistics
	};
};
