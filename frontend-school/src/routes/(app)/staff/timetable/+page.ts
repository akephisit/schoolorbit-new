export const _meta = {
	menu: {
		title: 'ตารางสอน',
		icon: 'CalendarDays',
		group: 'main',
		workspace: 'home',
		order: 2,
		user_type: 'staff'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
