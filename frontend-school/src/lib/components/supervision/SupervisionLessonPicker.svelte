<script lang="ts">
	import { untrack } from 'svelte';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	import { ArrowLeft, ArrowRight, Plus, X, Send } from '@lucide/svelte';
	import { authStore } from '#lib/stores/auth.js';
	import { userPermissions } from '#lib/stores/permissions.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import {
		getSupervisionBookingOptions,
		getSupervisionObservationTimetableOptions,
		type SupervisionBookingOptions,
		type SupervisionObservation,
		type SupervisionTimetableOption
	} from '#lib/api/supervision.js';
	import {
		addCivilDays,
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
		academicYearId,
		academicTermId,
		observationId,
		initialDate,
		observations = [],
		allowed,
		onsubmit,
		submitLabel = 'ส่งคำขอจอง',
		disabled = false
	}: {
		academicYearId: string;
		academicTermId: string;
		observationId?: string;
		initialDate?: string;
		observations?: SupervisionObservation[];
		allowed: boolean;
		disabled?: boolean;
		onsubmit: (selection: SupervisionLessonSelection) => Promise<void>;
		submitLabel?: string;
	} = $props();

	let weekStartDate = $state(bookingWeekStart());
	let options = $state<SupervisionBookingOptions | null>(null);
	let selected = $state<SupervisionTimetableOption[]>([]);
	let loading = $state(true);
	let error = $state('');
	let saveError = $state('');
	let success = $state('');
	let saving = $state(false);
	let selectionDigest = $state('');
	const request = new LatestRequest();
	let context = '';
	function ownerKey() {
		return `${identity}|${academicYearId}|${academicTermId}|${observationId ?? ''}|${allowed}`;
	}
	let epoch = 0;
	const identity = $derived.by(() => {
		void $authStore;
		void $userPermissions;
		return appIdentityKey();
	});
	const ordered = $derived(orderedBookingRange(selected));
	const rangeError = $derived(bookingRangeError(ordered));
	const stale = $derived(ordered.length > 0 && selectionDigest !== options?.sourceDigest);
	const days = $derived(
		Array.from({ length: 7 }, (_, index) => addCivilDays(weekStartDate, index))
	);
	const periods = $derived.by(() => {
		const rows: { key: string; label: string; start: string; end: string }[] = [];
		for (const item of options?.items ?? []) {
			const key = `${item.startTime}-${item.endTime}`;
			if (!rows.some((row) => row.key === key))
				rows.push({ key, label: item.periodName, start: item.startTime, end: item.endTime });
		}
		return rows.sort((a, b) => a.start.localeCompare(b.start));
	});
	const adjacent = $derived(
		adjacentBookingOptions(options?.items ?? [], ordered).filter(
			(item) => !bookedObservationFor(item, observations, observationId)
		)
	);

	$effect.pre(() => {
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
			weekStartDate = bookingWeekStart(initialDate);
			loading = allowed;
			if (allowed) void loadOptions();
		});
	});
	$effect(() => () => {
		epoch += 1;
		request.abort();
	});

	async function loadOptions() {
		if (!allowed) return;
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
						{ academicYearId, academicTermId, weekStartDate },
						{ signal: ticket.signal }
					);
			if (
				!request.isCurrent(ticket.revision) ||
				sourceEpoch !== epoch ||
				sourceContext !== ownerKey()
			)
				return;
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

	function changeWeek(offset: number) {
		if (saving) return;
		weekStartDate = offset === 0 ? bookingWeekStart() : addCivilDays(weekStartDate, offset * 7);
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
		if (saving || bookedObservationFor(item, observations, observationId)) return;
		selected = [item];
		selectionDigest = options?.sourceDigest ?? '';
		saveError = '';
		success = '';
	}
	function extendSelection(item: SupervisionTimetableOption) {
		if (saving || stale || bookingRangeError([...selected, item])) return;
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
		return (options?.items ?? []).filter(
			(item) => item.observedDate === date && item.startTime === start && item.endTime === end
		);
	}
	async function submit() {
		if (!allowed || disabled || saving || loading || rangeError || stale || !options) return;
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
			<Button variant="outline" size="sm" disabled={saving} onclick={() => changeWeek(-1)}
				><ArrowLeft class="size-4" />สัปดาห์ก่อน</Button
			>
			<Button variant="outline" size="sm" disabled={saving} onclick={() => changeWeek(0)}
				>สัปดาห์นี้</Button
			>
			<Button variant="outline" size="sm" disabled={saving} onclick={() => changeWeek(1)}
				>สัปดาห์ถัดไป<ArrowRight class="size-4" /></Button
			>
		</div>
	</div>
	{#if error}
		<PageState
			variant="error"
			title="โหลดคาบสอนไม่สำเร็จ"
			description={error}
			actionLabel="ลองใหม่"
			onaction={() => void loadOptions()}
		/>
	{:else if loading && !options}
		<PageSkeleton variant="table" />
	{:else if options?.items.length === 0}
		<PageState
			variant="empty"
			title="ไม่พบคาบสอนในสัปดาห์นี้"
			description="เลือกสัปดาห์อื่น หรือตรวจการจัดครูและห้องประจำชั้นในตารางสอน"
		/>
	{:else if options}
		<div class="overflow-x-auto rounded-xl border">
			<Table.Root class="table-fixed" style={`min-width: ${112 + periods.length * 192}px`}>
				<Table.Header
					><Table.Row
						><Table.Head class="sticky left-0 z-10 w-28 bg-background">วันที่</Table.Head
						>{#each periods as period (period.key)}<Table.Head
								class="w-48 text-center whitespace-normal break-words"
								><p>{period.label}</p>
								<p class="text-xs font-normal">
									{period.start.slice(0, 5)}–{period.end.slice(0, 5)}
								</p></Table.Head
							>{/each}</Table.Row
					></Table.Header
				>
				<Table.Body
					>{#each days as day (day)}<Table.Row
							><Table.Cell class="sticky left-0 z-10 bg-background align-top font-medium"
								>{bookingDateLabel(day)}</Table.Cell
							>{#each periods as period (period.key)}<Table.Cell
									class="w-48 p-1.5 align-top whitespace-normal"
									>{#each cellItems(day, period.start, period.end) as item (item.blockGroupId)}
										{@const booked = bookedObservationFor(item, observations, observationId)}
										{@const chosen = selected.some(
											(value) =>
												value.blockGroupId === item.blockGroupId && value.observedDate === day
										)}
										<div class="space-y-1">
											<button
												type="button"
												class={[
													'min-h-28 w-full whitespace-normal break-words rounded-lg border p-3 text-left transition hover:border-primary disabled:cursor-not-allowed disabled:opacity-60',
													chosen ? 'border-primary bg-primary/10' : ''
												].join(' ')}
												disabled={!!booked || saving || !item.homeroomId}
												aria-pressed={chosen}
												aria-label={`เลือก ${item.periodName} ${item.homeroomName} ${bookingDateLabel(day)}${selected.length ? ' เป็นคาบแรกของช่วงใหม่' : ''}`}
												onclick={() => startSelection(item)}
											>
												<p class="text-sm font-medium">
													{item.subjectName ?? item.learningGroupName}
												</p>
												<p class="mt-1 text-sm">{item.homeroomName}</p>
												{#if item.roomName}<p class="text-xs text-muted-foreground">
														{item.roomName}
													</p>{/if}
												{#if booked}<Badge variant="secondary" class="mt-2"
														>{supervisionStatusLabel(booked.status)}</Badge
													>{:else if chosen}<Badge class="mt-2">เลือกแล้ว</Badge>{/if}
											</button>
											{#if !stale && adjacent.some((value) => value.blockGroupId === item.blockGroupId && value.observedDate === day)}<Button
													variant="outline"
													size="sm"
													class="w-full"
													disabled={saving}
													aria-label={`เพิ่ม ${item.periodName} ${item.homeroomName}`}
													onclick={() => extendSelection(item)}
													><Plus class="size-4" />เพิ่มคาบติดกัน</Button
												>{:else if selected.length && !chosen && !booked && item.observedDate === ordered[0]?.observedDate && item.homeroomId !== ordered[0]?.homeroomId}<p
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
			</div>{/if}
		{#if success}<p role="status" class="text-sm font-medium text-primary">{success}</p>{/if}
		<LoadingButton
			loading={saving}
			loadingLabel="กำลังส่ง..."
			disabled={disabled || loading || !!error || !!rangeError || stale || !allowed}
			onclick={() => void submit()}><Send class="size-4" />{submitLabel}</LoadingButton
		>
	</div>
</div>
