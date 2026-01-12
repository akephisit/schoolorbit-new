/**
 * Menu Administration Page
 */

export const _meta = {
	menu: {
		title: 'จัดการเมนู',
		icon: 'Menu',
		group: 'settings',
		order: 1001,
		user_type: 'staff',
		permission: 'settings'
	}
};

export const load = async () => {
	return {
		title: 'จัดการเมนู'
	};
};
