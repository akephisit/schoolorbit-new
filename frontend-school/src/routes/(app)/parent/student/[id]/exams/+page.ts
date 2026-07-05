import type { PageLoad } from './$types';

const TITLE = 'ตารางสอบของลูก';

export const _meta = {
	access: {
		user_type: 'parent'
	},
	preview: {
		title: TITLE
	}
};

export const load: PageLoad = async ({ params }) => {
	return {
		title: TITLE,
		studentId: params.id
	};
};
