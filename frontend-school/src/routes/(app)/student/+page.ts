/**
 * Student Dashboard
 * Main dashboard for students (no specific permission required)
 */

export const _meta = {
	menu: {
		title: 'แดชบอร์ด',
		icon: 'LayoutDashboard',
		group: 'main',
		order: 1,
		user_type: 'student'
	}
};

export const load = async () => {
	return {
		title: 'Student Dashboard'
	};
};
