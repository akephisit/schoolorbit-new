import type { PageServerLoad } from './$types';

export const load: PageServerLoad = ({ setHeaders }) => {
	setHeaders({
		'content-security-policy': "frame-ancestors 'self' https:"
	});

	return { title: 'ปฏิทินโรงเรียน' };
};
