/**
 * Admission Management — รายการรอบรับสมัครทั้งหมด
 */

import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { waitForAdmissionAccess } from '#lib/admission/admission-access.js';
import { listRounds } from '#lib/api/admission.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import type { PageLoad } from './$types';

export const _meta = {
	academicContext: 'year_required' as const,
	menu: {
		title: 'รับสมัครนักเรียน',
		icon: 'ClipboardList',
		group: 'academic_admission',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ADMISSION
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const rounds = academicYearId
		? captureRouteLoad(
				waitForAdmissionAccess(PERMISSIONS.ADMISSION_READ_ALL).then((allowed) =>
					allowed ? listRounds(academicYearId, { requestFetch: fetch }) : []
				),
				'โหลดรอบรับสมัครไม่สำเร็จ'
			)
		: null;
	return { title: _meta.menu.title, academicYearId, rounds };
};
