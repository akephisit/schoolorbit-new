export const _meta = {
	menu: {
		title: 'ตารางเรียน',
		icon: 'CalendarDays',
		group: 'main',
		order: 2,
		user_type: 'student'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
