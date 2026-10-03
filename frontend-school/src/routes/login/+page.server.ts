import { publicRequestFromOrigin } from '$lib/api/client';
import { getRequiredPublicSchoolInfo } from '$lib/api/school';
import { captureRouteLoad } from '$lib/navigation/route-load';
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
