<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { SvelteDate } from 'svelte/reactivity';
	import { toast } from 'svelte-sonner';
	import { ChevronLeft, ChevronRight } from '@lucide/svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Alert from '#lib/components/ui/alert/index.js';
	import { PageState, LoadingButton } from '#lib/components/app-state/index.js';
	import CalendarMonthGrid from '#lib/components/calendar/CalendarMonthGrid.svelte';
	import {
		buildCalendarMonth,
		eventOverlapsDate,
		formatCalendarDate,
		formatCalendarMonth
	} from '#lib/utils/calendar.js';
	import {
		attendanceDays,
		saveAttendanceDays,
		type AttendanceDay,
		type AttendanceSettings
	} from '#lib/api/attendance.js';
	import type { RouteLoadResult } from '#lib/navigation/route-load.js';
	import { attendanceIdentity, type AttendanceRead } from './attendance-access.js';
	import {
		attendanceSettingsCalendar,
		attendanceMonthRange,
		type AttendanceCalendarEvents
	} from './attendance-settings-calendar.js';
	import AttendanceSelect from './AttendanceSelect.svelte';
	let {
		term,
		date,
		settings,
		busy,
		initialDays,
		initialCalendar,
		onsettings,
		onbusy
	}: {
		term: string;
		date: string;
		settings: AttendanceSettings;
		busy: boolean;
		initialDays: Promise<RouteLoadResult<AttendanceRead<AttendanceDay[]>>>;
		initialCalendar: Promise<RouteLoadResult<AttendanceRead<AttendanceCalendarEvents>>>;
		onsettings: (saved: AttendanceSettings) => void;
		onbusy: (value: boolean) => void;
	} = $props();
	let month = $state(untrack(() => date.slice(0, 7))),
		selectedDate = $state(untrack(() => date));
	let days = $state<AttendanceDay[]>([]),
		calendar = $state<AttendanceCalendarEvents>({ events: [], scope: 'own' });
	let dayLoading = $state(true),
		eventLoading = $state(true),
		dayError = $state(''),
		eventError = $state(''),
		saveError = $state(''),
		saving = $state(false),
		pendingAction = $state('');
	let note = $state(''),
		rangeStart = $state(untrack(() => date)),
		rangeEnd = $state(untrack(() => date));
	const dayRequest = new LatestRequest(),
		eventRequest = new LatestRequest();
	const initialDayTicket = dayRequest.begin(),
		initialEventTicket = eventRequest.begin();
	let disposed = false;
	onDestroy(() => {
		disposed = true;
		dayRequest.abort();
		eventRequest.abort();
	});
	const monthNumber = $derived(month.slice(5)),
		yearNumber = $derived(month.slice(0, 4));
	const yearOptions = $derived(
		Array.from({ length: 11 }, (_, i) => {
			const value = Number(yearNumber) - 5 + i;
			return { value: String(value), label: String(value + 543) };
		})
	);
	const monthOptions = Array.from({ length: 12 }, (_, i) => ({
		value: String(i + 1).padStart(2, '0'),
		label: formatCalendarMonth(`2026-${String(i + 1).padStart(2, '0')}-01`).split(' ')[0]
	}));
	const writable = $derived(!busy && !saving && !dayLoading && !dayError && !settings.archived);
	const selectedEvents = $derived(
		calendar.events.filter((event) => eventOverlapsDate(event, selectedDate))
	);
	function isCounted(value: string) {
		return (
			days.find((day) => day.date === value)?.counted ??
			settings.configuration.weekdays.includes(
				((new Date(value + 'T00:00:00Z').getUTCDay() + 6) % 7) + 1
			)
		);
	}
	const markers = $derived(
		dayLoading || dayError
			? []
			: buildCalendarMonth(month + '-01')
					.filter((cell) => cell.inCurrentMonth)
					.map((cell) => ({
						id: `processing-${cell.date}`,
						title: isCounted(cell.date) ? '✓ นับยอด' : '— ไม่นับยอด',
						startDate: cell.date,
						endDate: cell.date,
						allDay: false,
						categoryColor: isCounted(cell.date) ? 'var(--primary)' : 'var(--muted-foreground)'
					}))
	);
	$effect.pre(() => {
		const identityKey = $attendanceIdentity,
			daySource = initialDays,
			eventSource = initialCalendar;
		void daySource.then((result) => {
			if (
				disposed ||
				identityKey !== $attendanceIdentity ||
				!dayRequest.isCurrent(initialDayTicket.revision)
			)
				return;
			if (result.ok && result.data.identityKey === identityKey && result.data.resource)
				days = result.data.resource;
			else
				dayError = result.ok ? (result.data.error ?? 'ไม่มีสิทธิ์อ่านวันประมวลผล') : result.error;
			dayLoading = false;
		});
		void eventSource.then((result) => {
			if (
				disposed ||
				identityKey !== $attendanceIdentity ||
				!eventRequest.isCurrent(initialEventTicket.revision)
			)
				return;
			if (result.ok && result.data.identityKey === identityKey && result.data.resource)
				calendar = result.data.resource;
			else eventError = result.ok ? (result.data.error ?? 'ไม่มีสิทธิ์อ่านปฏิทิน') : result.error;
			eventLoading = false;
		});
	});
	async function loadDays() {
		const ticket = dayRequest.begin();
		dayLoading = true;
		dayError = '';
		days = [];
		try {
			const range = attendanceMonthRange(month);
			const result = await attendanceDays(term, range.from, range.to, { signal: ticket.signal });
			if (dayRequest.isCurrent(ticket.revision)) days = result;
		} catch (error) {
			if (dayRequest.isCurrent(ticket.revision))
				dayError = error instanceof Error ? error.message : 'โหลดวันประมวลผลไม่ได้';
		} finally {
			if (dayRequest.isCurrent(ticket.revision)) dayLoading = false;
		}
	}
	async function loadEvents() {
		const ticket = eventRequest.begin();
		eventLoading = true;
		eventError = '';
		calendar = { ...calendar, events: [] };
		try {
			const result = await attendanceSettingsCalendar(month, { signal: ticket.signal });
			if (eventRequest.isCurrent(ticket.revision)) calendar = result;
		} catch (error) {
			if (eventRequest.isCurrent(ticket.revision))
				eventError = error instanceof Error ? error.message : 'โหลดกิจกรรมไม่ได้';
		} finally {
			if (eventRequest.isCurrent(ticket.revision)) eventLoading = false;
		}
	}
	function selectMonth(value: string) {
		if (busy || saving || value === month) return;
		month = value;
		selectedDate = value + '-01';
		void loadDays();
		void loadEvents();
	}
	function moveMonth(offset: number) {
		const value = new Date(Date.UTC(Number(yearNumber), Number(monthNumber) - 1 + offset, 1))
			.toISOString()
			.slice(0, 7);
		selectMonth(value);
	}
	async function setDays(values: AttendanceDay[], action = 'day') {
		if (!writable || disposed) return;
		if (new TextEncoder().encode(note).length > 500) {
			saveError = 'หมายเหตุยาวเกินกำหนด กรุณาย่อข้อความ';
			return;
		}
		saving = true;
		pendingAction = action;
		onbusy(true);
		saveError = '';
		try {
			const saved = await saveAttendanceDays(term, {
				days: values,
				rowVersion: settings.rowVersion
			});
			if (disposed) return;
			onsettings(saved);
			days = [...days.filter((day) => !values.some((value) => value.date === day.date)), ...values];
			toast.success('บันทึกแล้ว');
		} catch (error) {
			if (!disposed) saveError = error instanceof Error ? error.message : 'บันทึกไม่ได้';
		} finally {
			if (!disposed) {
				saving = false;
				pendingAction = '';
				onbusy(false);
			}
		}
	}
	async function applyRange(counted: boolean) {
		try {
			if (!rangeStart || !rangeEnd || rangeStart > rangeEnd)
				throw new Error('เลือกช่วงวันที่ให้ถูกต้อง');
			const current = new SvelteDate(rangeStart + 'T00:00:00Z'),
				stop = new Date(rangeEnd + 'T00:00:00Z');
			if (!Number.isFinite(current.getTime()) || !Number.isFinite(stop.getTime()))
				throw new Error('เลือกช่วงวันที่ให้ถูกต้อง');
			const values: AttendanceDay[] = [];
			while (current <= stop && values.length <= 366) {
				values.push({ date: current.toISOString().slice(0, 10), counted, note });
				current.setUTCDate(current.getUTCDate() + 1);
			}
			if (values.length > 366) throw new Error('เลือกครั้งละไม่เกิน 366 วัน');
			await setDays(values, `range-${counted}`);
		} catch (error) {
			saveError = error instanceof Error ? error.message : 'ช่วงวันที่ไม่ถูกต้อง';
		}
	}
</script>

<Card.Root>
	<Card.Header
		><Card.Title><h2>ปฏิทินวันประมวลผล</h2></Card.Title><Card.Description
			>ดูกิจกรรมประกอบการเลือกวันนับยอด กิจกรรมในปฏิทินไม่เปลี่ยนการนับยอดโดยอัตโนมัติ</Card.Description
		></Card.Header
	>
	<Card.Content class="space-y-4">
		<div class="flex flex-wrap items-end gap-3">
			<div class="flex items-center gap-1">
				<Button
					type="button"
					variant="outline"
					size="icon"
					aria-label="เดือนก่อนหน้า"
					disabled={busy || saving}
					onclick={() => moveMonth(-1)}><ChevronLeft class="size-4" /></Button
				><Button
					type="button"
					variant="outline"
					size="icon"
					aria-label="เดือนถัดไป"
					disabled={busy || saving}
					onclick={() => moveMonth(1)}><ChevronRight class="size-4" /></Button
				>
			</div>
			<div class="w-40 space-y-2">
				<Label for="attendance-month">เดือน</Label><AttendanceSelect
					id="attendance-month"
					label="เดือน"
					value={monthNumber}
					options={monthOptions}
					disabled={busy || saving}
					onValueChange={(value) => selectMonth(yearNumber + '-' + value)}
				/>
			</div>
			<div class="w-28 space-y-2">
				<Label for="attendance-year">ปี</Label><AttendanceSelect
					id="attendance-year"
					label="ปี"
					value={yearNumber}
					options={yearOptions}
					disabled={busy || saving}
					onValueChange={(value) => selectMonth(value + '-' + monthNumber)}
				/>
			</div>
		</div>
		<p class="text-sm text-muted-foreground">
			{calendar.scope === 'school'
				? 'กิจกรรมจากปฏิทินโรงเรียน'
				: 'กิจกรรมจากปฏิทินที่คุณมีสิทธิ์เห็น'} · แถบ ✓ / — แสดงการนับยอดที่บันทึกแล้ว
		</p>
		{#if dayError}<PageState
				variant="error"
				title="โหลดวันประมวลผลไม่ได้"
				description={dayError}
				actionLabel="ลองโหลดวันประมวลผลใหม่"
				onaction={loadDays}
			/>{/if}
		{#if eventError}<PageState
				variant="error"
				title="โหลดกิจกรรมปฏิทินไม่ได้"
				description={eventError}
				actionLabel="ลองโหลดกิจกรรมใหม่"
				onaction={loadEvents}
			/>{/if}
		{#if dayLoading}<p role="status" class="text-sm text-muted-foreground">
				กำลังโหลดวันประมวลผล…
			</p>{/if}
		{#if eventLoading}<p role="status" class="text-sm text-muted-foreground">
				กำลังโหลดกิจกรรม…
			</p>{/if}
		<div aria-busy={dayLoading || eventLoading}>
			<CalendarMonthGrid
				monthDate={month + '-01'}
				events={calendar.events}
				annotations={markers}
				{selectedDate}
				hideMobileTimes
				onselect={(value) => {
					if (value.slice(0, 7) !== month) selectMonth(value.slice(0, 7));
					selectedDate = value;
				}}
				oneventselect={(_, value) => {
					selectedDate = value;
				}}
			/>
		</div>
		<section aria-label="วันที่เลือก" class="space-y-3 rounded-lg border p-4">
			<h3 class="font-medium">{formatCalendarDate(selectedDate)}</h3>
			<LoadingButton
				type="button"
				loading={saving && pendingAction === 'day'}
				variant={isCounted(selectedDate) ? 'default' : 'outline'}
				disabled={!writable || selectedDate.slice(0, 7) !== month}
				onclick={() => setDays([{ date: selectedDate, counted: !isCounted(selectedDate), note }])}
				>{isCounted(selectedDate) ? 'นับยอดวันนี้ ✓' : 'ไม่นับยอดวันนี้ —'}</LoadingButton
			>
			{#if !dayLoading && !dayError}<p class="text-sm text-muted-foreground">
					{days.find((day) => day.date === selectedDate)?.note ?? ''}
				</p>{/if}
			{#if selectedEvents.length}<ul class="space-y-3">
					{#each selectedEvents as event (event.id)}<li class="space-y-1 rounded-md border p-3">
							<p class="break-words font-medium">{event.title}</p>
							<p class="text-sm text-muted-foreground">
								{formatCalendarDate(event.startDate)} – {formatCalendarDate(event.endDate)} · {event.allDay
									? 'ทั้งวัน'
									: `${event.startTime?.slice(0, 5) ?? ''}–${event.endTime?.slice(0, 5) ?? ''}`}
							</p>
							{#if event.description}<p class="whitespace-pre-wrap break-words text-sm">
									{event.description}
								</p>{/if}{#if event.location}<p class="break-words text-sm">
									{event.location}
								</p>{/if}
						</li>{/each}
				</ul>{:else if !eventLoading && !eventError}<p class="text-sm text-muted-foreground">
					ไม่มีกิจกรรมในวันที่เลือก
				</p>{/if}
		</section>
		{#if saveError}<Alert.Root variant="destructive"
				><Alert.Title>บันทึกวันประมวลผลไม่ได้</Alert.Title><Alert.Description
					>{saveError}</Alert.Description
				></Alert.Root
			>{/if}
		<div class="space-y-2">
			<Label for="attendance-calendar-note">หมายเหตุวันหยุด / กิจกรรม</Label><Input
				id="attendance-calendar-note"
				bind:value={note}
				disabled={busy || saving || settings.archived}
			/>
		</div>
		<div class="flex flex-wrap items-end gap-3 border-t pt-4">
			<div class="min-w-0 space-y-2">
				<Label for="attendance-range-start">จาก</Label><DatePicker
					id="attendance-range-start"
					ariaLabel="จาก"
					bind:value={rangeStart}
					disabled={busy || saving || settings.archived}
				/>
			</div>
			<div class="min-w-0 space-y-2">
				<Label for="attendance-range-end">ถึง</Label><DatePicker
					id="attendance-range-end"
					ariaLabel="ถึง"
					bind:value={rangeEnd}
					disabled={busy || saving || settings.archived}
				/>
			</div>
			<LoadingButton
				type="button"
				loading={saving && pendingAction === 'range-true'}
				variant="outline"
				disabled={!writable}
				onclick={() => applyRange(true)}>นับช่วงนี้</LoadingButton
			><LoadingButton
				type="button"
				loading={saving && pendingAction === 'range-false'}
				variant="outline"
				disabled={!writable}
				onclick={() => applyRange(false)}>วันหยุด / ไม่นับช่วงนี้</LoadingButton
			>
		</div>
	</Card.Content>
</Card.Root>
