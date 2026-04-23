export const _meta = {
	menu: {
		title: 'ตั้งค่าโรงเรียน',
		icon: 'School',
		group: 'settings',
		order: 900,
		user_type: 'staff',
		permission: 'settings.update'
	}
};

export const load = async () => {
	return {
		title: 'ตั้งค่าโรงเรียน'
	};
};
