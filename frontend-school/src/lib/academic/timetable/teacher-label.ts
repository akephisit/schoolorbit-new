function teacherName(displayName: string): string {
	return displayName
		.trim()
		.replace(/^ครู\s*/, '')
		.replace(/^(?:นางสาว|นาง|นาย|ว่าที่ร้อยตรี|ดร\.|Mrs\.?|Mr\.?|Ms\.?)\s*/i, '')
		.trim()
		.replace(/\s+/g, ' ');
}

/** Compact timetable label; keep the source display name for selection and search. */
export function timetableTeacherLabel(displayName: string): string {
	const name = teacherName(displayName).split(' ')[0];
	return name && name !== '-' ? `ครู${name}` : '-';
}

/** PDF label includes the surname and uses the same teacher prefix. */
export function timetableTeacherFullLabel(displayName: string): string {
	const name = teacherName(displayName);
	return name && name !== '-' ? `ครู${name}` : '-';
}
