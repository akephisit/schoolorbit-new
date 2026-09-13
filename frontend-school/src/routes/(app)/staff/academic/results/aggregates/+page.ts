import { aggregateReadPermissions } from '$lib/academic/results/aggregate-access';

export const _meta = {
	academicContext: 'term_required' as const,
	access: { user_type: 'staff', permission: aggregateReadPermissions }
};
export const load = async () => ({ title: 'สรุปผลรายภาค' });
