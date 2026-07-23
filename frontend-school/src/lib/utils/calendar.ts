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

export const CALENDAR_WEEKDAY_LABELS = ['อา', 'จ', 'อ', 'พ', 'พฤ', 'ศ', 'ส'] as const;
export const CALENDAR_FALLBACK_COLOR = '#64748b';
const BUDDHIST_YEAR_OFFSET = 543;

export interface CalendarColorKeyEvent {
	id: string;
	startDate: string;
	endDate: string;
	categoryId?: string | null;
	categoryName?: string | null;
	categoryColor?: string | null;
}

export interface CalendarColorKeyItem {
	id: string;
	name: string;
	color: string;
}

export interface CalendarLayoutEvent {
	id: string;
	title: string;
	startDate: string;
	endDate: string;
	allDay?: boolean;
	startTime?: string | null;
	categoryColor?: string | null;
}

export interface CalendarWeekEventSegment<EventType extends CalendarLayoutEvent> {
	event: EventType;
	startColumn: number;
	span: number;
	lane: number;
	continuesFromPreviousWeek: boolean;
	continuesIntoNextWeek: boolean;
}

export interface CalendarMonthWeekLayout<EventType extends CalendarLayoutEvent> {
	cells: CalendarMonthCell[];
	segments: CalendarWeekEventSegment<EventType>[];
	hiddenEventCounts: number[];
}

export function toIsoDate(date: Date): string {
	return format(date, 'yyyy-MM-dd');
}

export function buildCalendarMonth(monthDate: string): CalendarMonthCell[] {
	const monthStart = startOfMonth(parseISO(monthDate));
	const gridStart = startOfWeek(monthStart, { weekStartsOn: 0 });

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

export function calendarGridRange(monthDate: string): { from: string; to: string } {
	const cells = buildCalendarMonth(monthDate);
	return {
		from: cells[0]?.date ?? monthRange(monthDate).from,
		to: cells.at(-1)?.date ?? monthRange(monthDate).to
	};
}

export function formatCalendarMonth(value: string): string {
	const date = parseISO(value);
	return `${format(date, 'MMMM', { locale: th })} ${date.getFullYear() + BUDDHIST_YEAR_OFFSET}`;
}

export function formatCalendarDate(value: string): string {
	const date = parseISO(value);
	return `${format(date, 'd MMM', { locale: th })} ${date.getFullYear() + BUDDHIST_YEAR_OFFSET}`;
}

export function eventOverlapsDate(
	event: { startDate: string; endDate: string },
	date: string
): boolean {
	return event.startDate <= date && event.endDate >= date;
}

export function buildCalendarColorKey(
	monthDate: string,
	events: CalendarColorKeyEvent[]
): CalendarColorKeyItem[] {
	const range = monthRange(monthDate);
	const categories = new Map<string, CalendarColorKeyItem>();
	let hasUncategorizedEvent = false;

	for (const event of events) {
		if (event.startDate > range.to || event.endDate < range.from) continue;

		if (event.categoryId && event.categoryName && event.categoryColor) {
			if (!categories.has(event.categoryId)) {
				categories.set(event.categoryId, {
					id: event.categoryId,
					name: event.categoryName,
					color: event.categoryColor
				});
			}
		} else {
			hasUncategorizedEvent = true;
		}
	}

	const items = [...categories.values()].sort((left, right) =>
		left.name.localeCompare(right.name, 'th')
	);

	if (hasUncategorizedEvent) {
		items.push({
			id: 'uncategorized',
			name: 'ไม่ระบุหมวดหมู่',
			color: CALENDAR_FALLBACK_COLOR
		});
	}

	return items;
}

export function buildCalendarMonthWeeks<EventType extends CalendarLayoutEvent>(
	monthDate: string,
	events: EventType[],
	maxVisibleLanes = 3
): CalendarMonthWeekLayout<EventType>[] {
	const cells = buildCalendarMonth(monthDate);
	const visibleLaneCount = Math.max(1, Math.floor(maxVisibleLanes));
	const preferredLaneByEventId = new Map<string, number>();
	const weeks: CalendarMonthWeekLayout<EventType>[] = [];

	for (let weekIndex = 0; weekIndex < 6; weekIndex += 1) {
		const weekCells = cells.slice(weekIndex * 7, weekIndex * 7 + 7);
		const weekStart = weekCells[0]?.date;
		const weekEnd = weekCells.at(-1)?.date;
		if (!weekStart || !weekEnd) continue;

		const visibleEvents = events
			.filter((event) => event.startDate <= weekEnd && event.endDate >= weekStart)
			.map((event) => {
				const startColumn = weekCells.findIndex((cell) => cell.date >= event.startDate);
				let endColumn = weekCells.length - 1;

				for (let column = weekCells.length - 1; column >= 0; column -= 1) {
					if ((weekCells[column]?.date ?? '') <= event.endDate) {
						endColumn = column;
						break;
					}
				}

				const normalizedStartColumn = startColumn === -1 ? 0 : startColumn;
				return {
					event,
					startColumn: normalizedStartColumn,
					endColumn,
					span: endColumn - normalizedStartColumn + 1
				};
			})
			.sort((left, right) => {
				const continuationOrder =
					Number(left.event.startDate >= weekStart) - Number(right.event.startDate >= weekStart);
				return (
					continuationOrder ||
					left.startColumn - right.startColumn ||
					right.span - left.span ||
					Number(right.event.allDay === true) - Number(left.event.allDay === true) ||
					(left.event.startTime ?? '').localeCompare(right.event.startTime ?? '') ||
					left.event.title.localeCompare(right.event.title)
				);
			});

		const occupiedColumnsByLane = Array.from({ length: visibleLaneCount }, () =>
			Array.from({ length: 7 }, () => false)
		);
		const hiddenEventCounts = Array.from({ length: 7 }, () => 0);
		const segments: CalendarWeekEventSegment<EventType>[] = [];

		for (const visibleEvent of visibleEvents) {
			const preferredLane = preferredLaneByEventId.get(visibleEvent.event.id);
			const laneCandidates = Array.from({ length: visibleLaneCount }, (_, lane) => lane);
			if (preferredLane !== undefined && preferredLane < visibleLaneCount) {
				laneCandidates.splice(preferredLane, 1);
				laneCandidates.unshift(preferredLane);
			}

			const lane = laneCandidates.find((candidateLane) => {
				for (let column = visibleEvent.startColumn; column <= visibleEvent.endColumn; column += 1) {
					if (occupiedColumnsByLane[candidateLane]?.[column]) return false;
				}
				return true;
			});

			if (lane === undefined) {
				for (let column = visibleEvent.startColumn; column <= visibleEvent.endColumn; column += 1) {
					hiddenEventCounts[column] = (hiddenEventCounts[column] ?? 0) + 1;
				}
				continue;
			}

			for (let column = visibleEvent.startColumn; column <= visibleEvent.endColumn; column += 1) {
				if (occupiedColumnsByLane[lane]) occupiedColumnsByLane[lane][column] = true;
			}

			preferredLaneByEventId.set(visibleEvent.event.id, lane);
			segments.push({
				event: visibleEvent.event,
				startColumn: visibleEvent.startColumn,
				span: visibleEvent.span,
				lane,
				continuesFromPreviousWeek: visibleEvent.event.startDate < weekStart,
				continuesIntoNextWeek: visibleEvent.event.endDate > weekEnd
			});
		}

		weeks.push({ cells: weekCells, segments, hiddenEventCounts });
	}

	return weeks;
}
