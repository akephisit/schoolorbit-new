export const _meta = {
	menu: {
		title: 'แดชบอร์ดผู้ปกครอง',
		icon: 'Users',
		group: 'main',
		workspace: 'home',
		order: 1,
		user_type: 'parent'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
