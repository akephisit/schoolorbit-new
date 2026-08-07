const timetablePdfDayOrder = ['MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT', 'SUN'] as const;
const defaultTimetablePdfDayValues = timetablePdfDayOrder.slice(0, 5);

export function resolveTimetablePdfDayValues(dayValues?: readonly string[]): string[] {
	if (!dayValues?.length) return [...defaultTimetablePdfDayValues];

	const requestedDays = new Set(dayValues);
	const resolvedDays = timetablePdfDayOrder.filter((day) => requestedDays.has(day));
	return resolvedDays.length > 0 ? resolvedDays : [...defaultTimetablePdfDayValues];
}
