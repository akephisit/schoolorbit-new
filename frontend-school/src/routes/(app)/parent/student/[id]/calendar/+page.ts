import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'parent'
	}
};

export const load: PageLoad = async ({ params }) => ({
	title: 'ปฏิทินของลูก',
	studentId: params.id
});
