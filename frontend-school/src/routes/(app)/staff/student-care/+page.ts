import type { PageLoad } from './$types';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare, careReadPermissions } from '#lib/features/student-care/access.js';

import { careOverview, careAssignments } from '#lib/api/student-care.js';
export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'ดูแลช่วยเหลือนักเรียน',
		icon: 'HeartHandshake',
		group: 'main',
		workspace: 'home',
		permission: [
			PERMISSIONS.STUDENT_CARE_READ_ASSIGNED,
			PERMISSIONS.STUDENT_CARE_READ_SCHOOL,
			PERMISSIONS.STUDENT_CARE_MANAGE_ASSIGNED,
			PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL,
			PERMISSIONS.STUDENT_CARE_READ_OWN,
			PERMISSIONS.STUDENT_CARE_CONFIG_MANAGE_SCHOOL,
			PERMISSIONS.STUDENT_CARE_SENSITIVE_READ_ASSIGNED,
			PERMISSIONS.STUDENT_CARE_SENSITIVE_READ_SCHOOL,
			PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_ASSIGNED,
			PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL
		],
		order: 46,
		user_type: 'staff'
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	return {
		term: url.searchParams.get('academicTermId'),
		overview: captureRouteLoad(
			readCare(careReadPermissions, () => careOverview({ requestFetch: fetch })),
			'โหลดภาพรวมไม่ได้'
		),
		assignments: captureRouteLoad(
			readCare(careReadPermissions, () => careAssignments({ limit: 10 }, { requestFetch: fetch })),
			'โหลดงานที่มอบหมายไม่ได้'
		)
	};
};
