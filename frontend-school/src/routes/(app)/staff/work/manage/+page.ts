import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '$lib/stores/permissions';
import { waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { listManageableWorkflowWindows } from '$lib/api/work';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', workflowManage: true }
};
export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	const windows = captureRouteLoad(
		waitForAuthenticatedUser().then((user) =>
			user?.user_type === 'staff' && get(can).hasWorkflowManage()
				? listManageableWorkflowWindows({}, { requestFetch: fetch })
				: []
		),
		'โหลดรอบงานไม่สำเร็จ'
	);
	return { title: 'จัดการรอบงาน', windows };
};
