import { publicRequestFromOrigin } from '#lib/api/client.js';
import { getRequiredPublicSchoolInfo } from '#lib/api/school.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = ({ fetch, url, setHeaders }) => {
	setHeaders({ 'cache-control': 'no-store' });
	return {
		identity: captureRouteLoad(
			getRequiredPublicSchoolInfo(publicRequestFromOrigin(fetch, url.origin)),
			'โหลดข้อมูลโรงเรียนไม่สำเร็จ'
		)
	};
};
