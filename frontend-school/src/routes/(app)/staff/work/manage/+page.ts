import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		workflowManage: true
	}
};

export const load: PageLoad = async () => {
	return {
		title: 'จัดการรอบงาน'
	};
};
