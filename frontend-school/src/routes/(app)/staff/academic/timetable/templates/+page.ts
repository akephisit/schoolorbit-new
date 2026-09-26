import type { PageLoad } from './$types';
import { listTimetableTemplates, listTimetableVersions } from '$lib/api/timetable';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSIONS } from '$lib/permissions/registry';

// ไม่อยู่ใน menu — เข้าจากปุ่ม "Templates" ในหน้า /staff/academic/timetable
const TITLE = 'Templates ตาราง';

export const _meta = {
	academicContext: 'term_required' as const,
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_TIMETABLE_READ_SCHOOL
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const academicTermId = url.searchParams.get('academicTermId')?.trim() || null;
	const requestedVersionId = url.searchParams.get('timetableVersionId')?.trim() || null;
	return {
		title: TITLE,
		academicYearId,
		academicTermId,
		requestedVersionId,
		templates: academicTermId
			? captureRouteLoad(
					listTimetableTemplates({ requestFetch: fetch }),
					'โหลดแม่แบบตารางสอนไม่สำเร็จ'
				)
			: null,
		versions: academicTermId
			? captureRouteLoad(
					listTimetableVersions(academicTermId, { requestFetch: fetch }),
					'โหลดรุ่นตารางสอนไม่สำเร็จ'
				)
			: null
	};
};
