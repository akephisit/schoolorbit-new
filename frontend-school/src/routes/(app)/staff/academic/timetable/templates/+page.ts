// ไม่อยู่ใน menu — เข้าจากปุ่ม "Templates" ในหน้า /staff/academic/timetable
const TITLE = 'Templates ตาราง';

export const _meta = {
	academicContext: 'term_required' as const
};

export const load = async () => {
	return {
		title: TITLE
	};
};
