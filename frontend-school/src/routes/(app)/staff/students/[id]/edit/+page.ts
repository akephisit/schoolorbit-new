/**
 * Edit Student Page (Admin/Staff)
 */

export const load = async ({ params }) => {
	return {
		title: 'แก้ไขข้อมูลนักเรียน',
		studentId: params.id
	};
};
