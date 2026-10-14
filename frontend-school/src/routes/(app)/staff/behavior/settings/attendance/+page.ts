import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readBehavior } from '#lib/features/behavior/access.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { behaviorRules, behaviorOptions } from '#lib/api/behavior.js';
import { bangkokDateTime } from '#lib/features/behavior/presentation.js';
const permissions = [PERMISSIONS.BEHAVIOR_MANAGE_SCHOOL];
export const _meta = {
	academicContext: 'term_required' as const,
	access: { user_type: 'staff', permission: permissions }
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const term = url.searchParams.get('academicTermId');
	const date = bangkokDateTime().date;
	return {
		term,
		date,
		rules: term
			? captureRouteLoad(
					readBehavior(permissions, async () => {
						const [rules, options] = await Promise.all([
							behaviorRules({ requestFetch: fetch }),
							behaviorOptions({ academicTermId: term, date }, { requestFetch: fetch })
						]);
						return { rules, options };
					}),
					'โหลดเกณฑ์ไม่ได้'
				)
			: null
	};
};
