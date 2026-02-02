/**
 * Student Management Page (Staff)
 */

export const _meta = {
	menu: {
		title: 'รายชื่อนักเรียน',
		icon: 'GraduationCap',
		group: 'academic',
		order: 5, // Top priority in academic
		user_type: 'staff',
		permission: 'student.read.all'
	}
};

export const load = async () => {
	return {
		title: 'จัดการนักเรียน'
	};
};
