import type { PageLoad } from './$types';
import { PERMISSIONS } from '$lib/permissions/registry';

const TITLE = 'จัดตารางสอบ';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL
	},
	preview: {
		title: TITLE
	}
};

export const load: PageLoad = async ({ params }) => {
	return {
		title: TITLE,
		roundId: params.id
	};
};
