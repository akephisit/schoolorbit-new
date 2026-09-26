import type { PageLoad } from './$types';
import { listExamRounds } from '$lib/api/examSchedule';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'ตารางสอบ',
		icon: 'CalendarClock',
		group: 'academic_assessment',
		workspace: 'academic',
		permission: PERMISSIONS.ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL,
		order: 30,
		user_type: 'staff'
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const academicTermId = url.searchParams.get('academicTermId')?.trim() || null;
	return {
		title: _meta.menu.title,
		academicYearId,
		academicTermId,
		rounds:
			academicYearId && academicTermId
				? captureRouteLoad(
						listExamRounds(academicTermId, { requestFetch: fetch }),
						'โหลดรายการรอบตารางสอบไม่สำเร็จ'
					)
				: null
	};
};
