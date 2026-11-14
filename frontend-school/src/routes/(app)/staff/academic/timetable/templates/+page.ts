import type { PageLoad } from './$types';
import { listTimetableTemplates, getTimetable } from '#lib/api/timetable.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';

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
	return {
		title: TITLE,
		academicYearId,
		academicTermId,
		templates: academicTermId
			? captureRouteLoad(
					listTimetableTemplates({ requestFetch: fetch }),
					'โหลดแม่แบบตารางสอนไม่สำเร็จ'
				)
			: null,
		timetable: academicTermId
			? captureRouteLoad(
					getTimetable(academicTermId, { requestFetch: fetch }),
					'โหลดตารางสอนไม่สำเร็จ'
				)
			: null
	};
};
