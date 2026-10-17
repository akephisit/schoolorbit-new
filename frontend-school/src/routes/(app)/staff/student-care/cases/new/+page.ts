import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare, careSensitiveManagePermissions } from '#lib/features/student-care/access.js';
import { careCaseSetup } from '#lib/features/student-care/case-setup.js';
import { careToday } from '#lib/features/student-care/presentation.js';
export const _meta = {
	academicContext: 'term_required' as const,
	access: { user_type: 'staff', permission: careSensitiveManagePermissions }
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const context = {
		term: url.searchParams.get('academicTermId') ?? '',
		date: careToday(),
		sourceAssignmentId: url.searchParams.get('sourceAssignmentId'),
		sourceVisitId: url.searchParams.get('sourceVisitId')
	};
	return {
		...context,
		initial: captureRouteLoad(
			readCare(careSensitiveManagePermissions, () =>
				careCaseSetup(context, { requestFetch: fetch })
			),
			'โหลดแบบเปิดการดูแลไม่ได้'
		)
	};
};
