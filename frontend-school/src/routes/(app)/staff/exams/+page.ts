export const _meta = {
	menu: {
		title: 'ตารางสอบ',
		icon: 'CalendarClock',
		group: 'main',
		workspace: 'home',
		order: 3,
		user_type: 'staff'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
