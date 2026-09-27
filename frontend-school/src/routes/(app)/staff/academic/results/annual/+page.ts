import type { PageLoad } from './$types';
import {
	aggregateReadPermissions,
	waitForAggregateReadAccess
} from '$lib/academic/results/aggregate-access';
import {
	listAnnualResultStudents,
	listAnnualResultRevisions,
	previewAnnualResult
} from '$lib/api/academicAggregates';
import { captureRouteLoad } from '$lib/navigation/route-load';

export const _meta = {
	academicContext: 'year_required' as const,
	access: { user_type: 'staff', permission: aggregateReadPermissions }
};
export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const selectedId = url.searchParams.get('studentAcademicYearId')?.trim() || null;
	const context = academicYearId ? { academicYearId } : null;
	if (!context)
		return {
			title: 'สรุปผลรายปี',
			context,
			selectedId,
			students: null,
			preview: null,
			history: null
		};
	const access = waitForAggregateReadAccess();
	const students = captureRouteLoad(
		access.then((allowed) =>
			allowed ? listAnnualResultStudents(context, { requestFetch: fetch }) : []
		),
		'โหลดรายชื่อผลรายปีไม่สำเร็จ'
	);
	const preview = selectedId
		? captureRouteLoad(
				access.then((allowed) =>
					allowed ? previewAnnualResult(selectedId, context, { requestFetch: fetch }) : null
				),
				'คำนวณผลรายปีไม่สำเร็จ'
			)
		: null;
	const history = selectedId
		? captureRouteLoad(
				access.then((allowed) =>
					allowed ? listAnnualResultRevisions(selectedId, context, { requestFetch: fetch }) : []
				),
				'โหลดประวัติผลรายปีไม่สำเร็จ'
			)
		: null;
	return { title: 'สรุปผลรายปี', context, selectedId, students, preview, history };
};
