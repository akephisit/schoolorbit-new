/**
 * Roles Management Page
 */

export const _meta = {
	menu: {
		title: 'จัดการบทบาท',
		icon: 'Shield',
		group: 'settings',
		order: 1000,
		permission: 'roles'
	}
};

export const load = async () => {
	return {
		title: 'จัดการบทบาท'
	};
};
