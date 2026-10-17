import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare, careManagePermissions } from '#lib/features/student-care/access.js';

import { careRound, careInstrument, careStudents, careStaff } from '#lib/api/student-care.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: careManagePermissions }
};
export const load: PageLoad = ({ fetch, params, depends }) => {
	depends('school:app-identity');
	return {
		id: params.id,
		initial: captureRouteLoad(
			readCare(careManagePermissions, async () => {
				const options = { requestFetch: fetch };
				const round = await careRound(params.id, options);
				const [instruments, students, staff] = await Promise.all([
					Promise.all(round.instrumentVersionIds.map((id) => careInstrument(id, options))),
					careStudents(
						{ academicTermId: round.academicTermId, date: round.rosterDate, limit: 50 },
						options
					),
					careStaff(options)
				]);
				return { round, instruments, students, staff };
			}),
			'โหลดแบบมอบหมายไม่ได้'
		)
	};
};
