import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare, careManagePermissions } from '#lib/features/student-care/access.js';

import { careStaff, careStudents } from '#lib/api/student-care.js';
import { careToday } from '#lib/features/student-care/presentation.js';
export const _meta = {
	academicContext: 'term_required' as const,
	access: { user_type: 'staff', permission: careManagePermissions }
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const term = url.searchParams.get('academicTermId') ?? '';
	const date = careToday();
	return {
		term,
		date,
		initial: captureRouteLoad(
			readCare(careManagePermissions, async () => {
				const options = { requestFetch: fetch };
				const [staff, students] = await Promise.all([
					careStaff(options),
					careStudents({ academicTermId: term, date, limit: 50 }, options)
				]);
				return { record: undefined, staff, students };
			}),
			'โหลดแบบเยี่ยมบ้านไม่ได้'
		)
	};
};
