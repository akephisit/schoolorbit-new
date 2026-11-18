<script lang="ts">
	import { ChevronLeft, ChevronRight } from '@lucide/svelte';
	import { addMonths, parseISO } from 'date-fns';
	import type { SupervisionObservation } from '#lib/api/supervision.js';
	import CalendarMonthGrid, {
		type CalendarDisplayEvent
	} from '#lib/components/calendar/CalendarMonthGrid.svelte';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { schoolCivilDate } from '#lib/supervision/booking-range.js';
	import {
		savedLessonLabels,
		supervisionStatusLabel
	} from '#lib/supervision/supervision-presentation.js';
	import {
		savedBookingsByDate,
		savedScheduleDateLabel,
		savedScheduleSummary,
		savedScheduleTime
	} from '#lib/supervision/supervision-schedule.js';
	import {
		calendarGridRange,
		formatCalendarMonth,
		monthRange,
		toIsoDate
	} from '#lib/utils/calendar.js';

	let {
		observations,
		selectedDate = $bindable(''),
		detailHref
	}: {
		observations: SupervisionObservation[];
		selectedDate?: string;
		detailHref: (observation: SupervisionObservation) => string;
	} = $props();

	const descriptionId = $props.id();
	let todayDate = $state(schoolCivilDate());
	const activeDate = $derived(selectedDate || todayDate);
	const bookingsByDate = $derived(savedBookingsByDate(observations));
	const bookingsForDay = $derived(bookingsByDate.get(activeDate) ?? []);
	const monthStart = $derived(monthRange(activeDate).from);
	const { events, pendingRequests } = $derived.by(() => {
		const events: CalendarDisplayEvent[] = [];
		const pendingRequests: CalendarDisplayEvent[] = [];
		const { from, to } = calendarGridRange(monthStart);
		for (const [date, bookings] of bookingsByDate) {
			if (date < from || date > to) continue;
			for (const { observation, periods } of bookings) {
				const pending = observation.status === 'requested';
				const event: CalendarDisplayEvent = {
					id: `${observation.id}-${date}`,
					title: observation.observedDisplayName ?? 'ครูผู้รับนิเทศ',
					startDate: date,
					endDate: date,
					allDay: false,
					startTime: savedScheduleTime(periods[0].observedAt),
					categoryColor: 'var(--color-primary)',
					pending
				};
				if (pending) pendingRequests.push(event);
				else events.push(event);
			}
		}
		return { events, pendingRequests };
	});

	function selectDate(date: string) {
		todayDate = schoolCivilDate();
		selectedDate = date;
	}

	function changeMonth(offset: number) {
		selectDate(toIsoDate(addMonths(parseISO(monthStart), offset)));
	}
</script>

<section class="min-w-0 space-y-3" aria-labelledby={`${descriptionId}-title`}>
	<div class="flex flex-wrap items-center justify-between gap-2">
		<h2 id={`${descriptionId}-title`} class="text-lg font-semibold">ปฏิทินนิเทศ</h2>
		<div
			class="grid w-full grid-cols-[auto_minmax(0,1fr)_auto_auto] items-center gap-1 sm:w-auto sm:grid-cols-[auto_8rem_auto_auto]"
			role="group"
			aria-label="เลื่อนเดือน"
		>
			<Button
				variant="outline"
				size="icon-sm"
				aria-label="เดือนก่อนหน้า"
				onclick={() => changeMonth(-1)}
			>
				<ChevronLeft />
			</Button>
			<p class="text-center text-sm font-medium" aria-live="polite">
				{formatCalendarMonth(monthStart)}
			</p>
			<Button
				variant="outline"
				size="icon-sm"
				aria-label="เดือนถัดไป"
				onclick={() => changeMonth(1)}
			>
				<ChevronRight />
			</Button>
			<Button variant="outline" size="sm" onclick={() => selectDate(schoolCivilDate())}
				>วันนี้</Button
			>
		</div>
	</div>

	<div class="grid items-start gap-3 xl:grid-cols-[minmax(0,1fr)_minmax(0,20rem)]">
		<div class="min-w-0 space-y-2">
			<div class="h-[33rem] sm:h-[40rem]">
				<CalendarMonthGrid
					monthDate={monthStart}
					{events}
					{pendingRequests}
					{todayDate}
					selectedDate={activeDate}
					onselect={selectDate}
					fillHeight
					hideMobileTimes
				/>
			</div>
			<div class="flex flex-wrap gap-2 text-xs text-muted-foreground" aria-label="สถานะนัดนิเทศ">
				<span class="inline-flex items-center gap-1.5"
					><span class="size-2 rounded-full bg-primary" aria-hidden="true"></span>นัดหมายแล้ว</span
				>
				<span class="inline-flex items-center gap-1.5"
					><span class="size-2 rounded-sm border border-dashed border-primary" aria-hidden="true"
					></span>รออนุมัติคำขอ</span
				>
			</div>
		</div>

		<Card.Root class="min-w-0 gap-3 p-4" role="region" aria-labelledby={`${descriptionId}-day`}>
			<h3 id={`${descriptionId}-day`} class="font-semibold" aria-live="polite">
				{savedScheduleDateLabel(activeDate)}
			</h3>
			{#if bookingsForDay.length}
				<ul class="divide-y border-t">
					{#each bookingsForDay as { observation, periods } (observation.id)}
						{@const schedule = savedScheduleSummary(periods)}
						{@const lesson = savedLessonLabels({ periods })}
						<li class="space-y-2 py-3 last:pb-0">
							<div class="flex flex-wrap items-start justify-between gap-1">
								<p class="min-w-0 font-semibold break-words">
									{observation.observedDisplayName ?? 'ครูผู้รับนิเทศ'}
								</p>
								<Badge variant={observation.status === 'requested' ? 'outline' : 'secondary'}>
									{supervisionStatusLabel(observation.status)}
								</Badge>
							</div>
							<div class="text-sm">
								<p class="font-medium text-primary">
									{schedule.periodTitle}{#if schedule.timeLabel}
										<span class="text-foreground tabular-nums">{` · ${schedule.timeLabel}`}</span>
									{/if}
								</p>
								{#if !schedule.timeLabel}
									<ul class="mt-1 space-y-1 text-sm tabular-nums">
										{#each schedule.periods as period (period.id)}
											<li>{period.label ?? 'ไม่ระบุคาบ'} · {period.timeLabel}</li>
										{/each}
									</ul>
								{/if}
								<p class="mt-1 break-words text-muted-foreground">
									{lesson.subject} · {lesson.homeroom}{lesson.room === '—'
										? ''
										: ` · ${lesson.room}`}
								</p>
							</div>
							<dl class="text-sm">
								<dt class="text-muted-foreground">ผู้ประเมิน</dt>
								<dd class="break-words">
									{#if observation.evaluators.length}
										<ul>
											{#each observation.evaluators as evaluator (evaluator.id)}
												<li>{evaluator.evaluatorDisplayName?.trim() || 'ไม่ทราบชื่อผู้ประเมิน'}</li>
											{/each}
										</ul>
									{:else}
										ยังไม่มอบหมายผู้ประเมิน
									{/if}
								</dd>
							</dl>
							<Button
								variant="ghost"
								size="sm"
								href={detailHref(observation)}
								data-sveltekit-preload-data="tap"
								aria-label={`รายละเอียด ${observation.observedDisplayName ?? 'รายการนิเทศ'}`}
							>
								รายละเอียด
							</Button>
						</li>
					{/each}
				</ul>
			{:else}
				<p class="text-sm text-muted-foreground">ไม่มีรายการจองนิเทศในวันนี้</p>
			{/if}
		</Card.Root>
	</div>
</section>
