import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params }) => {
	return {
		title: 'ข้อมูลนักเรียน',
		studentId: params.id
	};
};
