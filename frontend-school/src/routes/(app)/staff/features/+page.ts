/**
 * Feature Toggles Management Page
 */

export const _meta = {
	menu: {
		title: 'จัดการระบบงาน',
		icon: 'Zap',
		group: 'settings',
		order: 1000,
        user_type: 'staff',
		permission: 'settings'
	}
};

export const load = async () => {
	return {
		title: 'จัดการระบบงาน'
	};
};
