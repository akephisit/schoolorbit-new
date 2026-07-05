export const _meta = {
	menu: {
		title: 'ตารางสอบ',
		icon: 'CalendarClock',
		group: 'main',
		workspace: 'home',
		order: 4,
		user_type: 'student'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
