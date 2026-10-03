import { publicRequestFromOrigin } from '$lib/api/client';
import {
	getRequiredPublicSchoolInfo,
	getPublicSchoolStatistics,
	getPublicSchoolOrganization
} from '$lib/api/school';
import { captureRouteLoad } from '$lib/navigation/route-load';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = ({ fetch, url, setHeaders }) => {
	setHeaders({ 'cache-control': 'no-store' });
	const options = publicRequestFromOrigin(fetch, url.origin);
	return {
		title: 'เว็บไซต์โรงเรียน',
		description: 'ข้อมูลโรงเรียน สถิตินักเรียน ครู และโครงสร้างบริหาร',
		identity: captureRouteLoad(getRequiredPublicSchoolInfo(options), 'โหลดข้อมูลโรงเรียนไม่สำเร็จ'),
		statistics: captureRouteLoad(getPublicSchoolStatistics(options), 'โหลดสถิติโรงเรียนไม่สำเร็จ'),
		organization: captureRouteLoad(
			getPublicSchoolOrganization(options),
			'โหลดโครงสร้างบริหารไม่สำเร็จ'
		)
	};
};
