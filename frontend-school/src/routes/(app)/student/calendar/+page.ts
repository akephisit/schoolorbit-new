export const _meta = {
	menu: {
		title: 'ปฏิทิน',
		icon: 'CalendarDays',
		group: 'main',
		workspace: 'home',
		order: 3,
		user_type: 'student'
	}
};

export const load = async () => ({ title: _meta.menu.title });
