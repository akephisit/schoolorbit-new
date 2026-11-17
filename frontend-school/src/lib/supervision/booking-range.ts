import type {
	RequestSupervisionObservationRequest,
	SupervisionObservation,
	SupervisionTimetableOption
} from '#lib/api/supervision.js';

export type SupervisionLessonSelection = Pick<
	RequestSupervisionObservationRequest,
	'observedDate' | 'timetableBlockGroupIds' | 'expectedSourceDigest'
>;

export function schoolCivilDate(value = new Date()): string {
	return new Intl.DateTimeFormat('en-CA', {
		timeZone: 'Asia/Bangkok',
		year: 'numeric',
		month: '2-digit',
		day: '2-digit'
	}).format(value);
}

export function addCivilDays(value: string, days: number): string {
	const date = new Date(`${value}T00:00:00Z`);
	date.setUTCDate(date.getUTCDate() + days);
	return date.toISOString().slice(0, 10);
}

export function bookingWeekStart(value = schoolCivilDate()): string {
	const weekday = new Date(`${value}T00:00:00Z`).getUTCDay();
	return addCivilDays(value, -((weekday + 6) % 7));
}

export function bookingDateLabel(value: string): string {
	return new Intl.DateTimeFormat('th-TH', {
		timeZone: 'UTC',
		weekday: 'short',
		day: 'numeric',
		month: 'short'
	}).format(new Date(`${value}T00:00:00Z`));
}

function timeValue(value: string): number {
	const parts = value.split(':').map(Number);
	return (parts[0] ?? NaN) * 3600 + (parts[1] ?? NaN) * 60 + (parts[2] ?? 0);
}

export function orderedBookingRange(
	items: SupervisionTimetableOption[]
): SupervisionTimetableOption[] {
	return [...items].sort((a, b) => timeValue(a.startTime) - timeValue(b.startTime));
}

export function bookingRangeError(items: SupervisionTimetableOption[]): string | null {
	if (!items.length) return 'เลือกคาบสอนแรกจากตาราง';
	const ordered = orderedBookingRange(items);
	if (new Set(ordered.map((item) => item.blockGroupId)).size !== ordered.length)
		return 'เลือกคาบซ้ำไม่ได้';
	const first = ordered[0];
	if (!first.homeroomId) return 'คาบนี้ไม่มีห้องประจำชั้นที่ระบุชัดเจน';
	for (const [index, item] of ordered.entries()) {
		if (item.observedDate !== first.observedDate) return 'เลือกคาบในวันเดียวกันเท่านั้น';
		if (!item.homeroomId || item.homeroomId !== first.homeroomId)
			return 'ทุกคาบต้องเป็นห้องประจำชั้นเดียวกัน เช่น ม.3/2 ทุกคาบ';
		if (!(timeValue(item.endTime) > timeValue(item.startTime))) return 'เวลาของคาบไม่ถูกต้อง';
		if (index > 0 && timeValue(ordered[index - 1].endTime) !== timeValue(item.startTime))
			return 'คาบต้องต่อเนื่องกัน ห้ามข้ามคาบว่างหรือพักกลางวัน';
	}
	return null;
}

export function adjacentBookingOptions(
	items: SupervisionTimetableOption[],
	selected: SupervisionTimetableOption[]
): SupervisionTimetableOption[] {
	if (bookingRangeError(selected)) return [];
	return items.filter((item) => !bookingRangeError([...selected, item]));
}

export function bookedObservationFor(
	item: SupervisionTimetableOption,
	observations: SupervisionObservation[],
	excludedObservationId?: string
): SupervisionObservation | undefined {
	const startsAt = Date.parse(`${item.observedDate}T${item.startTime}+07:00`);
	const endsAt = Date.parse(`${item.observedDate}T${item.endTime}+07:00`);
	return observations.find(
		(observation) =>
			observation.id !== excludedObservationId &&
			observation.status !== 'cancelled' &&
			observation.periods.some((period) => {
				const savedStart = Date.parse(period.observedAt);
				return period.endsAt
					? savedStart < endsAt && Date.parse(period.endsAt) > startsAt
					: savedStart >= startsAt && savedStart < endsAt;
			})
	);
}
