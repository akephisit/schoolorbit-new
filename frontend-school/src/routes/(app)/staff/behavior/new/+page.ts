import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readBehavior, behaviorCreatePermissions } from '#lib/features/behavior/access.js';
import { behaviorItems, behaviorStudents } from '#lib/api/behavior.js';
import { bangkokDateTime } from '#lib/features/behavior/presentation.js';
export const _meta = {
	academicContext: 'term_required' as const,
	access: { user_type: 'staff', permission: behaviorCreatePermissions }
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const term = url.searchParams.get('academicTermId');
	const date = bangkokDateTime().date;
	return {
		term,
		date,
		initial: term
			? captureRouteLoad(
					readBehavior(behaviorCreatePermissions, async () => {
						const [items, roster] = await Promise.all([
							behaviorItems({ date }, { requestFetch: fetch }),
							behaviorStudents({ academicTermId: term, date, limit: 50 }, { requestFetch: fetch })
						]);
						return { items, roster };
					}),
					'โหลดแบบบันทึกไม่ได้'
				)
			: null
	};
};
