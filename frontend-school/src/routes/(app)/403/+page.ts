import type { PageLoad } from './$types';

export const load: PageLoad = async ({ url }) => {
	return {
		title: 'ไม่มีสิทธิ์เข้าถึง',
		from: url.searchParams.get('from')
	};
};
