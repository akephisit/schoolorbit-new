import type { PageLoad } from './$types';
import {
	aggregateReadPermissions,
	waitForAggregateReadAccess
} from '#lib/academic/results/aggregate-access.js';
import {
	listAggregateStudents,
	listAggregatePolicies,
	listTermAggregateRevisions,
	previewTermAggregate
} from '#lib/api/academicAggregates.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';

export const _meta = {
	academicContext: 'term_required' as const,
	access: { user_type: 'staff', permission: aggregateReadPermissions }
};
export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const academicTermId = url.searchParams.get('academicTermId')?.trim() || null;
	const selectedId = url.searchParams.get('studentAcademicYearId')?.trim() || null;
	const requestedPolicyId = url.searchParams.get('policyId')?.trim() || null;
	const context = academicYearId && academicTermId ? { academicYearId, academicTermId } : null;
	if (!context)
		return {
			title: 'สรุปผลรายภาค',
			context,
			selectedId,
			requestedPolicyId,
			students: null,
			policies: null,
			preview: null,
			history: null
		};
	const access = waitForAggregateReadAccess();
	const students = captureRouteLoad(
		access.then((allowed) =>
			allowed ? listAggregateStudents(context, { requestFetch: fetch }) : []
		),
		'โหลดรายชื่อผลสรุปไม่สำเร็จ'
	);
	const policies = captureRouteLoad(
		access.then((allowed) => (allowed ? listAggregatePolicies({ requestFetch: fetch }) : [])),
		'โหลดนโยบายผลสรุปไม่สำเร็จ'
	);
	const history = selectedId
		? captureRouteLoad(
				access.then((allowed) =>
					allowed ? listTermAggregateRevisions(selectedId, context, { requestFetch: fetch }) : []
				),
				'โหลดประวัติผลสรุปไม่สำเร็จ'
			)
		: null;
	const chosenPolicyId = requestedPolicyId
		? Promise.resolve(requestedPolicyId)
		: policies.then((result) => (result.ok ? (result.data[0]?.id ?? null) : null));
	const preview = selectedId
		? chosenPolicyId.then(async (policyId) => {
				if (!policyId) return { policyId: null, result: null };
				return {
					policyId,
					result: await captureRouteLoad(
						access.then((allowed) =>
							allowed
								? previewTermAggregate(selectedId, context, policyId, { requestFetch: fetch })
								: null
						),
						'คำนวณผลสรุปไม่สำเร็จ'
					)
				};
			})
		: null;
	return {
		title: 'สรุปผลรายภาค',
		context,
		selectedId,
		requestedPolicyId,
		students,
		policies,
		preview,
		history
	};
};
