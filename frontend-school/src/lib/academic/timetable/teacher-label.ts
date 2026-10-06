/** Compact timetable label; keep the source display name for selection and search. */
export function timetableTeacherLabel(displayName: string): string {
	const name = displayName
		.trim()
		.replace(/^ครู\s*/, '')
		.replace(/^(?:นางสาว|นาง|นาย|ว่าที่ร้อยตรี|ดร\.|Mrs\.?|Mr\.?|Ms\.?)\s*/i, '')
		.trim()
		.split(/\s+/)[0];
	return name && name !== '-' ? `ครู${name}` : '-';
}
