import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readBehavior, behaviorReadPermissions } from '#lib/features/behavior/access.js';
import {
	behaviorOverview,
	behaviorEntries,
	behaviorSummary,
	type BehaviorReportQuery
} from '#lib/api/behavior.js';
import { bangkokDateTime } from '#lib/features/behavior/presentation.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'พฤติกรรมนักเรียน',
		icon: 'HeartHandshake',
		group: 'main',
		workspace: 'home',
		permission: [
			PERMISSIONS.BEHAVIOR_READ_OWN,
			PERMISSIONS.BEHAVIOR_READ_ASSIGNED,
			PERMISSIONS.BEHAVIOR_READ_SCHOOL
		],
		order: 45,
		user_type: 'staff'
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const term = url.searchParams.get('academicTermId');
	const date = bangkokDateTime().date;
	const studentId = url.searchParams.get('studentId');
	const query: BehaviorReportQuery = {
		start: date.slice(0, 7) + '-01',
		end: date,
		academicTermId: term ?? undefined,
		limit: 50
	};
	return {
		title: _meta.menu.title,
		term,
		query,
		studentId,
		overview: captureRouteLoad(
			readBehavior(behaviorReadPermissions, () => behaviorOverview({ requestFetch: fetch })),
			'โหลดรอบคะแนนไม่ได้'
		),
		entries: term
			? captureRouteLoad(
					readBehavior(behaviorReadPermissions, () =>
						behaviorEntries(query, { requestFetch: fetch })
					),
					'โหลดรายการไม่ได้'
				)
			: null,
		summary: studentId
			? captureRouteLoad(
					readBehavior(behaviorReadPermissions, () =>
						behaviorSummary(studentId, {}, { requestFetch: fetch })
					),
					'โหลดคะแนนไม่ได้'
				)
			: null
	};
};
