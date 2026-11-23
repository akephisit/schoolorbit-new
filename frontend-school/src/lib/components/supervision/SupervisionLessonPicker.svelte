<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { page } from '$app/state';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { ArrowLeft, ArrowRight, Plus, X, Send } from '@lucide/svelte';
	import { getAcademicContextStore } from '#lib/academic-context/store.js';
	import { ACADEMIC_WEEKDAYS } from '#lib/academic-core/foundation-presentation.js';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import {
		getSupervisionBookingOptions,
		getSupervisionObservationTimetableOptions,
		type SupervisionBookingOptions,
		type SupervisionCycle,
		type SupervisionObservation,
		type SupervisionTimetableOption
	} from '#lib/api/supervision.js';
	import {
		addCivilDays,
		bookingAvailabilityLabel,
		bookingOptionUnavailable,
		clampedBookingWeek,
		cycleDateRangeError,
		reconcileBookingRange,
		withinCycleDates,
		adjacentBookingOptions,
		bookedObservationFor,
		bookingDateLabel,
		bookingRangeError,
		bookingWeekStart,
		orderedBookingRange,
		type SupervisionLessonSelection
	} from '#lib/supervision/booking-range.js';
	import { supervisionStatusLabel } from '#lib/supervision/supervision-presentation.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { LoadingButton, PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import * as Table from '#lib/components/ui/table/index.js';

	let {
		cycle,
		academicYearId,
		academicTermId,
		observationId,
		initialDate,
		observations = [],
		allowed,
		onreloadCycle,
		onsubmit,
		submitLabel = 'ส่งคำขอจอง',
		disabled = false
	}: {
		cycle: SupervisionCycle;
		academicYearId: string;
		academicTermId: string;
		observationId?: string;
		initialDate?: string;
		observations?: SupervisionObservation[];
		allowed: boolean;
		onreloadCycle?: () => Promise<void>;
		disabled?: boolean;
		onsubmit: (selection: SupervisionLessonSelection) => Promise<void>;
		submitLabel?: string;
	} = $props();

	const pickerId = $props.id();
	let weekStartDate = $state(bookingWeekStart());
	let options = $state<SupervisionBookingOptions | null>(null);
	let selected = $state<SupervisionTimetableOption[]>([]);
	let loading = $state(true);
	let error = $state('');
	let saveError = $state('');
	let success = $state('');
	let saving = $state(false);
	let reloadingCycle = $state(false);
	let selectionDigest = $state('');
	const request = new LatestRequest();
	const academicContext = getAcademicContextStore();
	const weekdayFormatter = new Intl.DateTimeFormat('th-TH', { timeZone: 'UTC', weekday: 'short' });
	const dateFormatter = new Intl.DateTimeFormat('th-TH', {
		timeZone: 'UTC',
		day: 'numeric',
		month: 'short'
	});
	let context = '';
	function ownerKey() {
		return `${appIdentityKey()}|${page.url.pathname}|${academicYearId}|${academicTermId}|${cycle.id}|${cycle.observationStartDate ?? ''}|${cycle.observationEndDate ?? ''}|${cycle.status}|${observationId ?? ''}|${allowed}`;
	}
	let epoch = 0;
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
	const ordered = $derived(orderedBookingRange(selected));
	const datesError = $derived(
		cycleDateRangeError(cycle.observationStartDate, cycle.observationEndDate)
	);
	const firstWeek = $derived(
		cycle.observationStartDate ? bookingWeekStart(cycle.observationStartDate) : ''
	);
	const lastWeek = $derived(
		cycle.observationEndDate ? bookingWeekStart(cycle.observationEndDate) : ''
	);
	const rangeError = $derived(
		bookingRangeError(ordered) ??
			ordered
				.map((item) =>
					bookingOptionUnavailable(item, cycle.observationStartDate, cycle.observationEndDate)
				)
				.find(Boolean) ??
			null
	);
	const stale = $derived(ordered.length > 0 && selectionDigest !== options?.sourceDigest);
	const schoolDays = $derived(
		$academicContext.options?.terms.find(
			(term) => term.id === academicTermId && term.academicYearId === academicYearId
		)?.schoolDays
	);
	const days = $derived.by(() => {
		const optionDates = new Set(options?.items.map((item) => item.observedDate));
		return ACADEMIC_WEEKDAYS.map((weekday, index) => ({
			weekday,
			date: addCivilDays(weekStartDate, index)
		}))
			.filter(({ weekday, date }) => schoolDays?.includes(weekday) ?? optionDates.has(date))
			.map(({ date }) => {
				const civilDay = new Date(`${date}T00:00:00Z`);
				return {
					date,
					weekdayLabel: weekdayFormatter.format(civilDay),
					dateLabel: dateFormatter.format(civilDay)
				};
			});
	});
	const visibleItems = $derived(
		(options?.items ?? []).filter((item) => days.some(({ date }) => date === item.observedDate))
	);
	const periods = $derived.by(() => {
		const rows: { key: string; label: string; start: string; end: string }[] = [];
		for (const item of visibleItems) {
			const key = `${item.startTime}-${item.endTime}`;
			if (!rows.some((row) => row.key === key))
				rows.push({ key, label: item.periodName, start: item.startTime, end: item.endTime });
		}
		return rows.sort((a, b) => a.start.localeCompare(b.start));
	});
	const adjacent = $derived(
		adjacentBookingOptions(options?.items ?? [], ordered).filter(
			(item) =>
				!bookingOptionUnavailable(item, cycle.observationStartDate, cycle.observationEndDate) &&
				!bookedObservationFor(item, observations, observationId)
		)
	);

	$effect.pre(() => {
		void identity;
		const key = ownerKey();
		untrack(() => {
			if (context === key) return;
			context = key;
			epoch += 1;
			request.abort();
			options = null;
			selected = [];
			selectionDigest = '';
			error = '';
			saveError = '';
			success = '';
			saving = false;
			reloadingCycle = false;
			weekStartDate = !datesError
				? clampedBookingWeek(cycle.observationStartDate!, cycle.observationEndDate!, initialDate)
				: bookingWeekStart(initialDate);
			loading = allowed && !datesError;
			if (allowed && !datesError) void loadOptions();
		});
	});
	onDestroy(() => {
		epoch += 1;
		request.abort();
	});

	async function loadOptions() {
		if (!allowed || datesError) return;
		const ticket = request.begin();
		const sourceEpoch = epoch;
		const sourceContext = ownerKey();
		loading = true;
		error = '';
		try {
			const result = observationId
				? await getSupervisionObservationTimetableOptions(observationId, weekStartDate, {
						signal: ticket.signal
					})
				: await getSupervisionBookingOptions(
						{ cycleId: cycle.id, academicYearId, academicTermId, weekStartDate },
						{ signal: ticket.signal }
					);
			if (
				!request.isCurrent(ticket.revision) ||
				sourceEpoch !== epoch ||
				sourceContext !== ownerKey()
			)
				return;
			if (selected.length) {
				const refreshed = reconcileBookingRange(
					selected,
					result.items,
					cycle.observationStartDate,
					cycle.observationEndDate
				);
				if (
					refreshed.length &&
					!refreshed.some((item) => bookedObservationFor(item, observations, observationId))
				) {
					selected = refreshed;
					selectionDigest = result.sourceDigest;
				} else {
					selected = [];
					selectionDigest = '';
					saveError = `${saveError ? `${saveError} ` : ''}ช่วงคาบเดิมไม่พร้อมจองหรือข้อมูลคาบเปลี่ยน กรุณาเลือกช่วงใหม่จากตารางล่าสุด`;
				}
			}
			options = result;
		} catch (failure) {
			if (
				!request.isCurrent(ticket.revision) ||
				sourceContext !== ownerKey() ||
				isAbortError(failure)
			)
				return;
			error = failure instanceof Error ? failure.message : 'โหลดคาบสอนไม่สำเร็จ';
		} finally {
			if (request.isCurrent(ticket.revision) && sourceContext === ownerKey()) loading = false;
		}
	}

	async function reloadCycle() {
		if (!onreloadCycle || reloadingCycle || saving) return;
		const sourceEpoch = epoch;
		const sourceContext = ownerKey();
		reloadingCycle = true;
		try {
			await onreloadCycle();
		} catch (failure) {
			if (sourceEpoch === epoch && sourceContext === ownerKey())
				saveError = failure instanceof Error ? failure.message : 'ตรวจช่วงวันที่ของรอบไม่สำเร็จ';
		} finally {
			if (sourceEpoch === epoch && sourceContext === ownerKey()) reloadingCycle = false;
		}
	}

	function changeWeek(offset: number) {
		if (saving || datesError) return;
		const candidate =
			offset === 0
				? clampedBookingWeek(cycle.observationStartDate!, cycle.observationEndDate!)
				: addCivilDays(weekStartDate, offset * 7);
		if (candidate < firstWeek || candidate > lastWeek || candidate === weekStartDate) return;
		weekStartDate = candidate;
		options = null;
		clearSelection();
		void loadOptions();
	}
	function clearSelection() {
		selected = [];
		selectionDigest = '';
		saveError = '';
		success = '';
	}
	function startSelection(item: SupervisionTimetableOption) {
		if (
			saving ||
			!allowed ||
			bookingOptionUnavailable(item, cycle.observationStartDate, cycle.observationEndDate) ||
			bookedObservationFor(item, observations, observationId)
		)
			return;
		selected = [item];
		selectionDigest = options?.sourceDigest ?? '';
		saveError = '';
		success = '';
	}
	function extendSelection(item: SupervisionTimetableOption) {
		if (
			saving ||
			!allowed ||
			stale ||
			bookingOptionUnavailable(item, cycle.observationStartDate, cycle.observationEndDate) ||
			bookedObservationFor(item, observations, observationId) ||
			bookingRangeError([...selected, item])
		)
			return;
		selected = orderedBookingRange([...selected, item]);
		saveError = '';
		success = '';
	}
	function removeEdge(index: number) {
		selected = ordered.filter((_, position) => position !== index);
		saveError = '';
		success = '';
	}
	function cellItems(date: string, start: string, end: string) {
		return visibleItems.filter(
			(item) => item.observedDate === date && item.startTime === start && item.endTime === end
		);
	}
	async function submit() {
		if (!allowed || disabled || saving || loading || datesError || rangeError || stale || !options)
			return;
		const sourceEpoch = epoch;
		const sourceContext = ownerKey();
		saving = true;
		saveError = '';
		success = '';
		try {
			await onsubmit({
				observedDate: ordered[0].observedDate,
				timetableBlockGroupIds: ordered.map((item) => item.blockGroupId),
				expectedSourceDigest: selectionDigest
			});
			if (sourceEpoch !== epoch || sourceContext !== ownerKey()) return;
			selected = [];
			success = 'ส่งคำขอสำเร็จ ดูสถานะได้ในรายการของฉัน';
		} catch (failure) {
			if (sourceEpoch !== epoch || sourceContext !== ownerKey()) return;
			saveError = failure instanceof Error ? failure.message : 'บันทึกไม่สำเร็จ';
		} finally {
			if (sourceEpoch === epoch && sourceContext === ownerKey()) saving = false;
		}
	}
</script>

<div class="min-w-0 space-y-4" data-testid="supervision-lesson-picker" aria-busy={loading}>
	<div class="flex flex-wrap items-center justify-between gap-3">
		<div>
			<p class="font-medium">
				{bookingDateLabel(weekStartDate)} – {bookingDateLabel(addCivilDays(weekStartDate, 6))}
			</p>
			<p class="text-sm text-muted-foreground">
				เลือกคาบแรก แล้วกด + เพื่อเพิ่มคาบติดกันของห้องเดียวกัน
			</p>
		</div>
		<div class="flex flex-wrap gap-1">
			<Button
				variant="outline"
				size="sm"
				disabled={saving || !!datesError || weekStartDate <= firstWeek}
				onclick={() => changeWeek(-1)}><ArrowLeft class="size-4" />สัปดาห์ก่อน</Button
			>
			<Button
				variant="outline"
				size="sm"
				disabled={saving || !!datesError}
				onclick={() => changeWeek(0)}>สัปดาห์นี้</Button
			>
			<Button
				variant="outline"
				size="sm"
				disabled={saving || !!datesError || weekStartDate >= lastWeek}
				onclick={() => changeWeek(1)}>สัปดาห์ถัดไป<ArrowRight class="size-4" /></Button
			>
		</div>
	</div>
	{#if datesError}<PageState title="รอผู้ดูแลกำหนดช่วงวันที่นิเทศ" description={datesError} />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดคาบสอนไม่สำเร็จ"
			description={error}
			actionLabel="ลองใหม่"
			onaction={() => void loadOptions()}
		/>
	{:else if loading && !options}
		<PageSkeleton variant="table" />
	{:else if options && periods.length === 0}
		<PageState
			variant="empty"
			title="ไม่พบคาบสอนในสัปดาห์นี้"
			description="เลือกสัปดาห์อื่น หรือตรวจการจัดครูและห้องประจำชั้นในตารางสอน"
		/>
	{:else if options}
		<p class="text-xs text-muted-foreground">
			ว่าง / รออนุมัติ / จองขนานได้: เลือกได้ · จองไม่ได้: คาบนี้มีนัดนิเทศที่ทับกัน
		</p>
		<div class="min-w-0 rounded-xl border" data-testid="supervision-booking-grid">
			<Table.Root
				role="table"
				class="grid w-(--booking-grid-width) grid-cols-[var(--booking-date-width)_repeat(var(--booking-period-count),minmax(0,1fr))] text-xs [--booking-date-width:3.5rem] sm:w-full sm:min-w-(--booking-grid-wide-width) sm:text-sm sm:[--booking-date-width:7rem]"
				style={`--booking-period-count: ${periods.length}; --booking-grid-width: ${3.5 + periods.length * 6}rem; --booking-grid-wide-width: ${7 + periods.length * 12}rem`}
			>
				<Table.Header role="rowgroup" class="col-span-full grid grid-cols-subgrid"
					><Table.Row role="row" class="col-span-full grid grid-cols-subgrid"
						><Table.Head
							role="columnheader"
							scope="col"
							class="sticky left-0 z-10 flex h-auto min-h-10 items-center bg-background px-1.5 sm:px-2"
							>วันที่</Table.Head
						>{#each periods as period (period.key)}<Table.Head
								role="columnheader"
								scope="col"
								class="flex h-auto flex-col justify-center px-1 text-center whitespace-normal wrap-anywhere sm:px-2"
								><p>{period.label}</p>
								<p class="text-[0.6875rem] font-normal sm:text-xs">
									{period.start.slice(0, 5)}–{period.end.slice(0, 5)}
								</p></Table.Head
							>{/each}</Table.Row
					></Table.Header
				>
				<Table.Body role="rowgroup" class="col-span-full grid auto-rows-fr grid-cols-subgrid"
					>{#each days as { date: day, weekdayLabel, dateLabel } (day)}<Table.Row
							role="row"
							class="col-span-full grid grid-cols-subgrid"
							><Table.Head
								role="rowheader"
								scope="row"
								class="sticky left-0 z-10 h-auto bg-background px-1.5 py-2 font-medium sm:px-2"
								><time datetime={day}>
									<span class="block">{weekdayLabel}</span>
									<span class="block font-normal text-muted-foreground">{dateLabel}</span>
								</time>{#if !withinCycleDates(day, cycle.observationStartDate, cycle.observationEndDate)}<p
										class="mt-1 text-[0.6875rem] font-normal whitespace-normal text-muted-foreground sm:text-xs"
									>
										นอกช่วงนิเทศ
									</p>{/if}</Table.Head
							>{#each periods as period (period.key)}<Table.Cell
									role="cell"
									class="space-y-1.5 p-1 whitespace-normal sm:p-1.5"
									>{#each cellItems(day, period.start, period.end) as item (item.blockGroupId)}
										{@const detailsId = `${pickerId}-${day}-${item.blockGroupId}`}
										{@const booked = bookedObservationFor(item, observations, observationId)}
										{@const unavailable = bookingOptionUnavailable(
											item,
											cycle.observationStartDate,
											cycle.observationEndDate
										)}
										{@const chosen = selected.some(
											(value) =>
												value.blockGroupId === item.blockGroupId && value.observedDate === day
										)}
										{@const canExtend =
											!stale &&
											adjacent.some(
												(value) =>
													value.blockGroupId === item.blockGroupId && value.observedDate === day
											)}
										<div class="space-y-1">
											<div class="grid grid-cols-[minmax(0,1fr)_auto] grid-rows-[auto_auto_auto]">
												<button
													type="button"
													class="col-span-full row-span-full grid min-h-20 min-w-0 grid-cols-subgrid grid-rows-subgrid rounded-lg border p-1.5 text-left leading-snug whitespace-normal wrap-anywhere transition hover:border-primary focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-ring disabled:cursor-not-allowed disabled:opacity-60 aria-pressed:border-primary aria-pressed:bg-primary/10 sm:min-h-28 sm:p-3"
													disabled={!allowed ||
														!!booked ||
														!!unavailable ||
														saving ||
														!item.homeroomId}
													aria-pressed={chosen}
													aria-label={`เลือก ${item.periodName} ${item.homeroomName} ${bookingDateLabel(day)}${selected.length ? ' เป็นคาบแรกของช่วงใหม่' : ''}`}
													aria-describedby={detailsId}
													onclick={() => startSelection(item)}
												>
													<span
														id={detailsId}
														class="col-span-full row-span-full grid grid-cols-subgrid grid-rows-subgrid"
													>
														<span class="col-span-full row-start-1 font-medium">
															{item.subjectName ?? item.learningGroupName}
														</span>
														<span class="col-start-1 row-start-2 mt-0.5 self-center sm:mt-1">
															<span class="block">{item.homeroomName}</span>
															{#if item.roomName}<span
																	class="block text-[0.6875rem] text-muted-foreground sm:text-xs"
																>
																	{item.roomName}
																</span>{/if}
														</span>
														<span class="col-span-full row-start-3 mt-1 sm:mt-2"
															><Badge
																variant={item.bookingAvailability.blocked
																	? 'destructive'
																	: 'outline'}
																class="max-w-full px-1 text-[0.6875rem] leading-snug whitespace-normal sm:px-2 sm:text-xs"
																>{bookingAvailabilityLabel(item)}</Badge
															>
															{#if unavailable}<span
																	class="mt-1 block text-[0.6875rem] text-muted-foreground sm:text-xs"
																>
																	{unavailable}
																</span>{/if}
															{#if booked}<Badge
																	variant="secondary"
																	class="mt-1 max-w-full px-1 text-[0.6875rem] leading-snug whitespace-normal sm:mt-2 sm:px-2 sm:text-xs"
																	>{supervisionStatusLabel(booked.status)}</Badge
																>{/if}
														</span>
													</span>
												</button>
												{#if canExtend}<Button
														variant="outline"
														size="icon-sm"
														class="col-start-2 row-start-2 ms-1 me-1.5 self-center sm:me-3"
														disabled={saving}
														aria-label={`เพิ่ม ${item.periodName} ${item.homeroomName}`}
														onclick={() => extendSelection(item)}
														><Plus class="size-4" aria-hidden="true" /></Button
													>{/if}
											</div>
											{#if !canExtend && selected.length && !chosen && !booked && item.observedDate === ordered[0]?.observedDate && item.homeroomId !== ordered[0]?.homeroomId}<p
													class="px-1 text-xs text-muted-foreground"
												>
													เพิ่มไม่ได้: คนละห้องประจำชั้น
												</p>{/if}
										</div>
									{/each}</Table.Cell
								>{/each}</Table.Row
						>{/each}</Table.Body
				>
			</Table.Root>
		</div>
	{/if}
	<div
		class="space-y-3 rounded-xl border bg-muted/20 p-4"
		data-testid="supervision-booking-selection"
	>
		{#if ordered.length}
			<div class="flex items-start justify-between gap-3">
				<div>
					<p class="font-semibold">
						{bookingDateLabel(ordered[0].observedDate)} · {ordered[0].homeroomName}
					</p>
					<p class="text-sm">
						{ordered.length} คาบต่อเนื่อง · {ordered[0].startTime.slice(0, 5)}–{ordered[
							ordered.length - 1
						].endTime.slice(0, 5)}
					</p>
				</div>
				<Button variant="ghost" size="sm" disabled={saving} onclick={clearSelection}
					>ล้างทั้งหมด</Button
				>
			</div>
			<div class="flex flex-wrap gap-2">
				{#each ordered as item, index (item.blockGroupId)}<Badge
						variant="outline"
						class="max-w-full gap-2 py-1.5 whitespace-normal"
						><span class="min-w-0 break-words"
							>{item.periodName} · {item.subjectName ?? item.learningGroupName}</span
						>{#if index === 0 || index === ordered.length - 1}<button
								type="button"
								disabled={saving}
								class="shrink-0"
								aria-label={`นำ ${item.periodName} ออกจากช่วง`}
								onclick={() => removeEdge(index)}><X class="size-3.5" /></button
							>{/if}</Badge
					>{/each}
			</div>
		{:else}<p class="text-sm text-muted-foreground">
				ยังไม่ได้เลือกคาบ · เลือกหนึ่งคาบหรือหลายคาบติดกันในวันและห้องเดียวกัน
			</p>{/if}
		{#if rangeError && ordered.length}<p role="alert" class="text-sm text-destructive">
				{rangeError}
			</p>{/if}
		{#if stale}<p role="alert" class="text-sm text-destructive">
				ตารางสอนเปลี่ยนแล้ว กรุณาเลือกช่วงคาบใหม่จากข้อมูลล่าสุด ร่างเดิมยังแสดงอยู่ด้านบน
			</p>{/if}
		{#if saveError}<div role="alert" class="space-y-2 text-sm text-destructive">
				<p>{saveError}</p>
				<Button
					variant="outline"
					size="sm"
					disabled={saving || loading}
					onclick={() => void loadOptions()}>ตรวจตารางสอนล่าสุด</Button
				>
				{#if onreloadCycle}<Button
						variant="outline"
						size="sm"
						disabled={saving || loading || reloadingCycle}
						onclick={() => void reloadCycle()}>ตรวจช่วงวันที่ของรอบ</Button
					>{/if}
			</div>{/if}
		{#if success}<p role="status" class="text-sm font-medium text-primary">{success}</p>{/if}
		<LoadingButton
			loading={saving}
			loadingLabel="กำลังส่ง..."
			disabled={disabled || loading || !!datesError || !!error || !!rangeError || stale || !allowed}
			onclick={() => void submit()}><Send class="size-4" />{submitLabel}</LoadingButton
		>
	</div>
</div>
