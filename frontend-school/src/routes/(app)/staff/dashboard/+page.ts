/**
 * Dashboard Page - Main landing page
 */

export const _meta = {
	menu: {
		title: 'หน้าหลัก',
		icon: 'Home',
		group: 'main',
		order: 1,
		permission: null // Everyone can see dashboard
	}
};

export const load = async () => {
	return {
		title: 'หน้าหลัก'
	};
};
