<script lang="ts" generics="Booking extends CalendarScheduleBooking">
	import { ChevronLeft, ChevronRight } from '@lucide/svelte';
	import { addMonths, parseISO } from 'date-fns';
	import type { Snippet } from 'svelte';
	import { PageState } from '#lib/components/app-state/index.js';
	import CalendarMonthGrid, {
		type CalendarDisplayEvent
	} from '#lib/components/calendar/CalendarMonthGrid.svelte';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { Skeleton } from '#lib/components/ui/skeleton/index.js';
	import { schoolCivilDate } from '#lib/supervision/booking-range.js';
	import { supervisionStatusLabel } from '#lib/supervision/supervision-presentation.js';
	import {
		calendarBookingsByDate,
		calendarScheduleSummary,
		savedScheduleDateLabel,
		savedScheduleTime,
		type CalendarScheduleBooking
	} from '#lib/supervision/supervision-schedule.js';
	import {
		calendarGridRange,
		formatCalendarMonth,
		monthRange,
		toIsoDate
	} from '#lib/utils/calendar.js';
	import { cn } from '#lib/utils.js';

	let {
		bookings,
		cycles,
		academicLabel = '',
		view,
		selectedDate = $bindable(''),
		monthDate,
		ondatechange,
		loading = false,
		error = false,
		onretry,
		submitted,
		bookingNotes,
		bookingActions
	}: {
		bookings: Booking[];
		cycles: { id: string; title: string }[];
		academicLabel?: string;
		view: 'requests' | 'evaluate' | 'staff';
		selectedDate?: string;
		monthDate?: string;
		ondatechange?: (date: string) => void;
		loading?: boolean;
		error?: boolean;
		onretry?: () => void;
		submitted?: (booking: Booking) => boolean;
		bookingNotes?: Snippet<[Booking, Booking['periods'][number][]]>;
		bookingActions?: Snippet<[Booking]>;
	} = $props();

	const descriptionId = $props.id();
	let todayDate = $state(schoolCivilDate());
	const activeDate = $derived(selectedDate || todayDate);
	const bookingsByDate = $derived(calendarBookingsByDate(bookings));
	const bookingsForDay = $derived(bookingsByDate.get(activeDate) ?? []);
	const monthStart = $derived(monthDate ?? monthRange(activeDate).from);
	const { events, pendingRequests, cycleIds } = $derived.by(() => {
		const events: CalendarDisplayEvent[] = [];
		const pendingRequests: CalendarDisplayEvent[] = [];
		const bookingCycleIds: string[] = [];
		const { from, to } = calendarGridRange(monthStart);
		for (const [date, dayBookings] of bookingsByDate) {
			if (date < from || date > to) continue;
			for (const { booking, periods } of dayBookings) {
				bookingCycleIds.push(booking.cycleId);
				const pending = booking.status === 'requested';
				const event: CalendarDisplayEvent = {
					id: `${booking.id}-${date}`,
					title: booking.observedDisplayName ?? 'ครูผู้รับนิเทศ',
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
		return { events, pendingRequests, cycleIds: new Set(bookingCycleIds) };
	});
	const cycleTitles = $derived(
		new Map(
			(cycleIds.size === 0 && cycles.length === 1
				? cycles
				: cycles.filter(({ id }) => cycleIds.has(id))
			).map(({ id, title }) => [id, title])
		)
	);
	const selectedDayEmptyLabel = $derived(
		view === 'staff'
			? 'ไม่มีคาบนิเทศในวันที่เลือก'
			: view === 'evaluate'
				? 'ไม่มีรายการนิเทศที่ได้รับมอบหมายในวันนี้'
				: 'ไม่มีรายการจองนิเทศในวันนี้'
	);

	function selectDate(date: string) {
		todayDate = schoolCivilDate();
		if (ondatechange) ondatechange(date);
		else selectedDate = date;
	}

	function changeMonth(offset: number) {
		selectDate(toIsoDate(addMonths(parseISO(monthStart), offset)));
	}
</script>

{#snippet statusLegend()}
	<div
		class="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground"
		role="group"
		aria-label="สถานะนัดนิเทศ"
	>
		<span class="inline-flex items-center gap-1.5">
			<span class="size-2 rounded-full bg-primary" aria-hidden="true"></span>
			{view === 'staff' ? 'ยืนยันแล้ว' : 'นัดหมายแล้ว'}
		</span>
		<span class="inline-flex items-center gap-1.5">
			<span class="size-2 rounded-sm border border-dashed border-primary" aria-hidden="true"></span>
			รออนุมัติคำขอ
		</span>
	</div>
{/snippet}

<section
	class="min-w-0 space-y-4"
	aria-label={view === 'staff' ? 'ปฏิทินรายเดือน' : undefined}
	aria-labelledby={view === 'staff' ? undefined : `${descriptionId}-title`}
>
	<div class="grid items-center gap-3 sm:grid-cols-[minmax(0,1fr)_auto]">
		{#if view === 'staff'}
			{@render statusLegend()}
		{:else}
			<div class="min-w-0">
				<h2 id={`${descriptionId}-title`} class="text-lg font-semibold text-balance sm:text-xl">
					{academicLabel ? `ปฏิทินนิเทศการสอน ${academicLabel}` : 'ปฏิทินนิเทศการสอน'}
				</h2>
				{#if cycleTitles.size}
					<p class="mt-0.5 text-sm break-words text-muted-foreground">
						รอบนิเทศ {[...cycleTitles.values()].join(' · ')}
					</p>
				{/if}
			</div>
		{/if}
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
	{#if view === 'requests'}{@render statusLegend()}{/if}

	{#if error}
		<PageState
			variant="error"
			title="โหลดปฏิทินนิเทศไม่สำเร็จ"
			actionLabel="ลองโหลดปฏิทินอีกครั้ง"
			onaction={onretry}
		/>
	{:else}
		<div
			class="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(0,20rem)]"
			aria-busy={loading}
			data-testid={view === 'staff' ? 'supervision-calendar-schedule' : undefined}
		>
			<div class={cn('min-w-0 sm:h-[40rem]', view === 'staff' ? 'h-[27rem]' : 'h-[33rem]')}>
				{#if loading}
					<div class="h-full" role="status" aria-label="กำลังโหลดปฏิทินนิเทศ">
						<Skeleton class="h-full w-full rounded-xl" />
					</div>
				{:else}
					<CalendarMonthGrid
						monthDate={monthStart}
						{events}
						{pendingRequests}
						{todayDate}
						selectedDate={activeDate}
						onselect={selectDate}
						fillHeight
						hideMobileTimes
						hidePendingPrefix
					/>
				{/if}
			</div>

			<Card.Root class="min-w-0 gap-3 p-4" role="region" aria-labelledby={`${descriptionId}-day`}>
				<div class="space-y-1">
					<p class="text-xs font-medium text-muted-foreground">กำหนดการประจำวันที่เลือก</p>
					<h3 id={`${descriptionId}-day`} class="font-semibold" aria-live="polite">
						{savedScheduleDateLabel(activeDate)}
					</h3>
					{#if view === 'staff' && bookingsForDay.length > 0}
						<p class="text-xs text-muted-foreground">เวลาไทย</p>
					{/if}
				</div>
				{#if loading}
					<Skeleton class="h-4 w-40 max-w-full" />
					<Skeleton class="h-4 w-28 max-w-full" />
				{:else if bookingsForDay.length}
					<ul class="divide-y border-t">
						{#each bookingsForDay as { booking, periods } (booking.id)}
							{@const schedule = calendarScheduleSummary(periods)}
							{@const cycleTitle = cycleTitles.get(booking.cycleId)}
							<li class="space-y-2 py-3 last:pb-0">
								<div class="flex flex-wrap items-start justify-between gap-1">
									<div class="min-w-0">
										<p class="flex items-baseline gap-1.5 font-semibold wrap-anywhere">
											{#if view !== 'staff' && booking.status === 'requested'}
												<span
													class="size-2 shrink-0 rounded-sm border border-dashed border-primary"
													aria-hidden="true"
												></span>
											{/if}
											<span class="min-w-0">{booking.observedDisplayName ?? 'ครูผู้รับนิเทศ'}</span>
										</p>
										{#if (view === 'staff' || cycleIds.size > 1) && cycleTitle}
											<p class="text-xs wrap-anywhere text-muted-foreground">{cycleTitle}</p>
										{/if}
									</div>
									{#if view === 'staff'}
										<Badge variant={booking.status === 'requested' ? 'outline' : 'secondary'}>
											{booking.status === 'requested' ? 'รออนุมัติคำขอ' : 'ยืนยันแล้ว'}
										</Badge>
									{:else if booking.status === 'requested'}
										<span class="sr-only">{supervisionStatusLabel(booking.status)}</span>
									{:else if booking.status !== 'confirmed'}
										<div class="flex flex-wrap gap-1">
											<Badge variant="secondary">{supervisionStatusLabel(booking.status)}</Badge>
											{#if submitted?.(booking)}
												<Badge
													variant="outline"
													class="border-success/30 bg-success/10 text-success">ส่งผลแล้ว</Badge
												>
											{/if}
										</div>
									{/if}
								</div>
								<div class="text-sm">
									<p class="font-medium">
										{schedule.periodTitle}{#if schedule.timeLabel}
											<span class="tabular-nums text-muted-foreground"
												>{` · ${schedule.timeLabel}`}</span
											>
										{/if}
									</p>
									{#if !schedule.timeLabel}
										<ul class="mt-1 space-y-1 text-sm tabular-nums">
											{#each schedule.periods as period (period.id)}
												<li>{period.label ?? 'ไม่ระบุคาบ'} · {period.timeLabel}</li>
											{/each}
										</ul>
									{/if}
									{@render bookingNotes?.(booking, periods)}
								</div>
								<dl class="text-sm">
									<dt class="text-muted-foreground">ผู้ประเมิน</dt>
									<dd class="wrap-anywhere">
										{#if booking.evaluatorDisplayNames.length}
											<ul>
												{#each booking.evaluatorDisplayNames as name, index (index)}
													<li>{name.trim() || 'ไม่ทราบชื่อผู้ประเมิน'}</li>
												{/each}
											</ul>
										{:else}
											ยังไม่มอบหมายผู้ประเมิน
										{/if}
									</dd>
								</dl>
								{#if bookingActions}
									<div class="flex flex-wrap items-center gap-2">
										{@render bookingActions(booking)}
									</div>
								{/if}
							</li>
						{/each}
					</ul>
				{:else}
					<p class="text-sm text-muted-foreground">{selectedDayEmptyLabel}</p>
				{/if}
			</Card.Root>
		</div>
	{/if}
</section>
