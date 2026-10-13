import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readBehavior } from '#lib/features/behavior/access.js';
import { loadOwnBehavior } from '#lib/features/behavior/own-route.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
export const _meta = {
	menu: {
		title: 'พฤติกรรมและคะแนน',
		icon: 'HeartHandshake',
		group: 'main',
		workspace: 'home',
		permission: PERMISSIONS.BEHAVIOR_READ_OWN,
		order: 25,
		user_type: 'student'
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	return {
		initial: captureRouteLoad(
			readBehavior([PERMISSIONS.BEHAVIOR_READ_OWN], () => loadOwnBehavior(fetch, url, 'student')),
			'โหลดข้อมูลไม่ได้'
		),
		url: url.href
	};
};
