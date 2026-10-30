import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readBehavior } from './access.js';
import { behaviorSummary, behaviorEntries } from '#lib/api/behavior.js';
import {
	listMyAcademicContextOptions,
	listParentAcademicContextOptions
} from '#lib/api/academic-context.js';
import { getOwnParentProfile, type ChildDto } from '#lib/api/parents.js';
import { bangkokDateTime } from './presentation.js';
export async function loadOwnBehavior(
	requestFetch: typeof fetch,
	url: URL,
	userType: 'student' | 'parent',
	signal?: AbortSignal
) {
	const user = await waitForAuthenticatedUser();
	if (!user || user.user_type !== userType || !get(can).has(PERMISSIONS.BEHAVIOR_READ_OWN))
		return null;
	const context =
		userType === 'student'
			? await listMyAcademicContextOptions(signal, { requestFetch, signal })
			: await listParentAcademicContextOptions(signal, { requestFetch, signal });
	const term =
		context.terms.find((t) => t.id === url.searchParams.get('academicTermId')) ??
		context.terms.find((t) => t.id === context.defaultAcademicTermId) ??
		context.terms.at(-1);
	const children: ChildDto[] =
		userType === 'parent' && term
			? (await getOwnParentProfile(term.academicYearId, { requestFetch, signal })).children
			: [];
	const studentId =
		userType === 'student'
			? user.id
			: (children.find((c) => c.id === url.searchParams.get('studentId'))?.id ?? children[0]?.id);
	const date = bangkokDateTime().date;
	const query = {
		start: date.slice(0, 7) + '-01',
		end: date,
		studentId,
		academicTermId: term?.id,
		limit: 50
	};
	const permissions = [PERMISSIONS.BEHAVIOR_READ_OWN];
	return {
		terms: context.terms,
		children,
		termId: term?.id ?? null,
		studentId: studentId ?? null,
		query,
		summary: studentId
			? captureRouteLoad(
					readBehavior(permissions, () => behaviorSummary(studentId, {}, { requestFetch, signal })),
					'โหลดคะแนนไม่ได้'
				)
			: null,
		entries:
			studentId && term
				? captureRouteLoad(
						readBehavior(permissions, () => behaviorEntries(query, { requestFetch, signal })),
						'โหลดประวัติไม่ได้'
					)
				: null
	};
}
