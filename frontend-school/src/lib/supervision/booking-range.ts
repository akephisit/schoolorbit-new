import type {
	RequestSupervisionObservationRequest,
	SupervisionObservation,
	SupervisionTimetableOption
} from '#lib/api/supervision.js';

export type SupervisionLessonSelection = Pick<
	RequestSupervisionObservationRequest,
	'observedDate' | 'timetableBlockGroupIds' | 'expectedSourceDigest'
>;

const schoolDateFormatter = new Intl.DateTimeFormat('en-CA', {
	timeZone: 'Asia/Bangkok',
	year: 'numeric',
	month: '2-digit',
	day: '2-digit'
});

export function schoolCivilDate(value = new Date()): string {
	return schoolDateFormatter.format(value);
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

export function cycleDateRangeError(start?: string | null, end?: string | null): string | null {
	if (!start || !end) return 'กำหนดวันเริ่มต้นและวันสุดท้ายที่ให้นิเทศก่อน';
	for (const date of [start, end]) {
		const parsed = new Date(`${date}T00:00:00Z`);
		if (
			!/^\d{4}-\d{2}-\d{2}$/.test(date) ||
			!Number.isFinite(parsed.getTime()) ||
			parsed.toISOString().slice(0, 10) !== date
		)
			return 'ระบุวันที่ให้ถูกต้อง';
	}
	return start > end ? 'วันสุดท้ายต้องไม่อยู่ก่อนวันเริ่มต้น' : null;
}

export function withinCycleDates(
	date: string,
	start?: string | null,
	end?: string | null
): boolean {
	return !cycleDateRangeError(start, end) && Boolean(start && end && date >= start && date <= end);
}

export function clampedBookingWeek(start: string, end: string, date = schoolCivilDate()): string {
	return bookingWeekStart(date < start ? start : date > end ? end : date);
}

export function cycleDateRangeLabel(start?: string | null, end?: string | null): string {
	if (cycleDateRangeError(start, end)) return 'ยังไม่กำหนดช่วงวันที่นิเทศ';
	const formatter = new Intl.DateTimeFormat('th-TH', {
		timeZone: 'UTC',
		day: 'numeric',
		month: 'short',
		year: 'numeric'
	});
	return `${formatter.format(new Date(`${start}T00:00:00Z`))} – ${formatter.format(new Date(`${end}T00:00:00Z`))}`;
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

export function bookingOptionUnavailable(
	item: SupervisionTimetableOption,
	start?: string | null,
	end?: string | null
): string | null {
	if (!withinCycleDates(item.observedDate, start, end)) return 'อยู่นอกช่วงวันที่นิเทศ';
	return item.bookingAvailability.blocked ? 'คาบนี้มีนัดนิเทศที่ทับกัน จองเพิ่มไม่ได้' : null;
}

export function bookingAvailabilityLabel(item: SupervisionTimetableOption): string {
	const { pendingCount, confirmedCount, blocked } = item.bookingAvailability;
	const labels = [];
	if (pendingCount) labels.push(`รออนุมัติ ${pendingCount}`);
	if (confirmedCount) labels.push(`นัดหมาย ${confirmedCount}`);
	if (blocked) labels.push('จองไม่ได้');
	else if (confirmedCount) labels.push('จองขนานได้');
	return labels.join(' · ') || 'ว่าง';
}

export function reconcileBookingRange(
	selected: SupervisionTimetableOption[],
	items: SupervisionTimetableOption[],
	start?: string | null,
	end?: string | null
): SupervisionTimetableOption[] {
	const current: SupervisionTimetableOption[] = [];
	for (const old of selected) {
		const item = items.find(
			(value) => value.blockGroupId === old.blockGroupId && value.observedDate === old.observedDate
		);
		if (
			!item ||
			bookingOptionUnavailable(item, start, end) ||
			item.homeroomId !== old.homeroomId ||
			item.learningGroupId !== old.learningGroupId ||
			item.startTime !== old.startTime ||
			item.endTime !== old.endTime
		)
			return [];
		current.push(item);
	}
	return bookingRangeError(current) ? [] : orderedBookingRange(current);
}
