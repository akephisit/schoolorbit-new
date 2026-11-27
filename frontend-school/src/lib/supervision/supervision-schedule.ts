import type {
	SupervisionObservationPeriod,
	SupervisionObservationStatus
} from '#lib/api/supervision.js';
import { schoolCivilDate } from '#lib/supervision/booking-range.js';

const dateFormatter = new Intl.DateTimeFormat('th-TH', {
	timeZone: 'UTC',
	weekday: 'short',
	day: 'numeric',
	month: 'short',
	year: 'numeric'
});
const timeFormatter = new Intl.DateTimeFormat('th-TH', {
	timeZone: 'Asia/Bangkok',
	hour: '2-digit',
	minute: '2-digit',
	hourCycle: 'h23'
});

export function savedScheduleDateLabel(date: string): string {
	return dateFormatter.format(new Date(`${date}T00:00:00Z`));
}

export function savedScheduleTime(value: string): string | null {
	const time = Date.parse(value);
	return Number.isFinite(time) ? timeFormatter.format(time) : null;
}

function periodDate(value: string): string | null {
	const date = new Date(value);
	return Number.isFinite(date.getTime()) ? schoolCivilDate(date) : null;
}

export function savedPeriodLabel(period: SupervisionObservationPeriod): string | null {
	return (
		period.lessonSnapshot.periodLabel?.trim() || period.manualLesson?.periodLabel?.trim() || null
	);
}

export type CalendarSchedulePeriod = Pick<
	SupervisionObservationPeriod,
	'id' | 'observedAt' | 'endsAt' | 'position'
> & { periodLabel?: string | null };

export type CalendarScheduleBooking = {
	id: string;
	cycleId: string;
	observedDisplayName?: string | null;
	status: SupervisionObservationStatus | 'confirmed';
	evaluatorDisplayNames: string[];
	periods: CalendarSchedulePeriod[];
};

function orderedPeriods<Period extends CalendarSchedulePeriod>(periods: Period[]): Period[] {
	return [...periods].sort(
		(a, b) => Date.parse(a.observedAt) - Date.parse(b.observedAt) || a.position - b.position
	);
}

function knownEnd(period: CalendarSchedulePeriod): string | null {
	return period.endsAt && Date.parse(period.endsAt) > Date.parse(period.observedAt)
		? period.endsAt
		: null;
}

function periodTimeLabel(period: CalendarSchedulePeriod): string {
	const start = savedScheduleTime(period.observedAt);
	if (!start) return 'ไม่ระบุเวลาเริ่ม';
	const end = knownEnd(period);
	if (!end) return `${start} น. · ไม่ระบุเวลาสิ้นสุด`;
	const endDate = periodDate(end);
	if (endDate && endDate !== periodDate(period.observedAt)) {
		return `${start} น. ถึง ${savedScheduleDateLabel(endDate)} ${savedScheduleTime(end)} น.`;
	}
	return `${start}–${savedScheduleTime(end)} น.`;
}

export function savedScheduleSummary(periods: SupervisionObservationPeriod[]) {
	return calendarScheduleSummary(
		periods.map((period) => ({ ...period, periodLabel: savedPeriodLabel(period) }))
	);
}

export function calendarScheduleSummary(periods: CalendarSchedulePeriod[]) {
	const ordered = orderedPeriods(periods);
	const individualPeriods = ordered.map((period) => ({
		id: period.id,
		date: periodDate(period.observedAt),
		label: period.periodLabel?.trim() || null,
		timeLabel: periodTimeLabel(period)
	}));
	const dates = [...new Set(individualPeriods.map((period) => period.date))].filter(
		(date): date is string => date !== null
	);
	const first = ordered[0];
	const last = ordered.at(-1);
	const end = last && knownEnd(last);
	const labels = individualPeriods.map((period) => period.label);
	const numbers = labels.map((label) => Number(label?.match(/^คาบ\s*(?:ที่\s*)?(\d+)$/u)?.[1]));
	const contiguous =
		first &&
		end &&
		dates.length === 1 &&
		periodDate(end) === dates[0] &&
		ordered.every((period, index) => {
			const next = ordered[index + 1];
			const periodEnd = knownEnd(period);
			return periodEnd && (!next || Date.parse(periodEnd) === Date.parse(next.observedAt));
		});
	const consecutive = contiguous && numbers.every((number, index) => number === numbers[0] + index);
	const periodTitle = !first
		? 'ไม่พบข้อมูลคาบ'
		: ordered.length === 1
			? (labels[0] ?? 'ไม่ระบุคาบ')
			: consecutive
				? `คาบ ${numbers[0]}–${numbers.at(-1)}`
				: labels.some(Boolean)
					? labels.map((label) => label ?? 'ไม่ระบุคาบ').join(' · ')
					: `${ordered.length} คาบ`;
	return {
		dates,
		periodTitle,
		timeLabel:
			first && ordered.length === 1
				? individualPeriods[0].timeLabel
				: consecutive
					? `${savedScheduleTime(first.observedAt)}–${savedScheduleTime(end)} น.`
					: null,
		periods: individualPeriods
	};
}

export function calendarBookingsByDate<Booking extends CalendarScheduleBooking>(
	bookings: Booking[]
) {
	type Period = Booking['periods'][number];
	const days = new Map<string, { booking: Booking; periods: Period[] }[]>();
	for (const booking of bookings) {
		const bookingDates = new Map<string, Period[]>();
		for (const period of orderedPeriods<Period>(booking.periods)) {
			const date = periodDate(period.observedAt);
			if (!date) continue;
			const periods = bookingDates.get(date) ?? [];
			periods.push(period);
			bookingDates.set(date, periods);
		}
		for (const [date, periods] of bookingDates) {
			const dayBookings = days.get(date) ?? [];
			dayBookings.push({ booking, periods });
			days.set(date, dayBookings);
		}
	}
	for (const bookings of days.values()) {
		bookings.sort(
			(a, b) => Date.parse(a.periods[0].observedAt) - Date.parse(b.periods[0].observedAt)
		);
	}
	return days;
}
