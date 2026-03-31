export const _meta = {
	menu: {
		title: 'ลงทะเบียนกิจกรรม',
		icon: 'Users',
		group: 'main',
		order: 5,
		user_type: 'student'
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
