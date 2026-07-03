import {
	addDays,
	endOfMonth,
	format,
	isSameMonth,
	parseISO,
	startOfMonth,
	startOfWeek
} from 'date-fns';
import { th } from 'date-fns/locale';

export interface CalendarMonthCell {
	date: string;
	dayNumber: number;
	inCurrentMonth: boolean;
}

export function toIsoDate(date: Date): string {
	return format(date, 'yyyy-MM-dd');
}

export function buildCalendarMonth(monthDate: string): CalendarMonthCell[] {
	const monthStart = startOfMonth(parseISO(monthDate));
	const gridStart = startOfWeek(monthStart, { weekStartsOn: 1 });

	return Array.from({ length: 42 }, (_, index) => {
		const date = addDays(gridStart, index);
		return {
			date: toIsoDate(date),
			dayNumber: Number(format(date, 'd')),
			inCurrentMonth: isSameMonth(date, monthStart)
		};
	});
}

export function monthRange(monthDate: string): { from: string; to: string } {
	const parsed = parseISO(monthDate);
	return {
		from: toIsoDate(startOfMonth(parsed)),
		to: toIsoDate(endOfMonth(parsed))
	};
}

export function formatCalendarDate(value: string): string {
	return format(parseISO(value), 'd MMM yyyy', { locale: th });
}

export function eventOverlapsDate(
	event: { startDate: string; endDate: string },
	date: string
): boolean {
	return event.startDate <= date && event.endDate >= date;
}
