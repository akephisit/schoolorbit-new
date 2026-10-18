import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare, careSensitiveManagePermissions } from '#lib/features/student-care/access.js';

import { careCase, careStaff } from '#lib/api/student-care.js';
import { careToday } from '#lib/features/student-care/presentation.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: careSensitiveManagePermissions }
};
export const load: PageLoad = ({ fetch, params, url, depends }) => {
	depends('school:app-identity');
	return {
		id: params.id,
		term: url.searchParams.get('academicTermId') ?? '',
		date: careToday(),
		initial: captureRouteLoad(
			readCare(careSensitiveManagePermissions, async () => {
				const [record, staff] = await Promise.all([
					careCase(params.id, { requestFetch: fetch }),
					careStaff({ requestFetch: fetch })
				]);
				return { record, staff };
			}),
			'โหลดแผนการดูแลไม่ได้'
		)
	};
};
