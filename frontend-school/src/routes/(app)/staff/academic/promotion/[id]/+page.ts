import { PERMISSIONS } from '$lib/permissions/registry';
import type { PageLoad } from './$types';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL }
};
export const load: PageLoad = ({ params }) => ({ runId: params.id, title: 'ตรวจรอบเลื่อนชั้น' });
