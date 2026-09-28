/**
 * Create New Admission Round Page
 */

import { PERMISSIONS } from '$lib/permissions/registry';
import { waitForAdmissionAccess } from '$lib/admission/admission-access';
import { lookupAcademicYears, lookupGradeLevels } from '$lib/api/lookup';
import { captureRouteLoad } from '$lib/navigation/route-load';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.ADMISSION_MANAGE_ALL
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const preferredYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const years = captureRouteLoad(
		waitForAdmissionAccess(PERMISSIONS.ADMISSION_MANAGE_ALL).then((allowed) =>
			allowed ? lookupAcademicYears({ activeOnly: false }, { requestFetch: fetch }) : []
		),
		'โหลดปีการศึกษาไม่สำเร็จ'
	);
	const grades = years.then((result) => {
		if (!result.ok) return { ok: true as const, data: { yearId: '', rows: [] }, error: null };
		const year =
			result.data.find((item) => item.id === preferredYearId) ??
			result.data.find((item) => item.status === 'active') ??
			result.data[0];
		return year
			? captureRouteLoad(
					lookupGradeLevels({ academicYearId: year.id }, { requestFetch: fetch }).then((rows) => ({
						yearId: year.id,
						rows
					})),
					'โหลดระดับชั้นไม่สำเร็จ'
				)
			: { ok: true as const, data: { yearId: '', rows: [] }, error: null };
	});
	return { title: 'สร้างรอบรับสมัครใหม่', preferredYearId, years, grades };
};
