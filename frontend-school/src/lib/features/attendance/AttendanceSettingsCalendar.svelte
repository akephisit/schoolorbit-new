<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { beforeNavigate } from '$app/navigation';
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
	import CalendarMonthGrid, {
		type CalendarDayState
	} from '#lib/components/calendar/CalendarMonthGrid.svelte';
	import CalendarDayTimelineDialog from '#lib/components/calendar/CalendarDayTimelineDialog.svelte';
	import {
		buildCalendarMonth,
		eventOverlapsDate,
		formatCalendarMonth
	} from '#lib/utils/calendar.js';
	import {
		attendanceDays,
		attendanceSettings,
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
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	interface PendingDay {
		day: AttendanceDay;
		revision: number;
	}
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
	let dayDialogOpen = $state(false),
		pendingDays = $state<PendingDay[]>([]),
		calendarBusy = $state(false);
	let editRevision = 0;
	let days = $state<AttendanceDay[]>([]),
		calendar = $state<AttendanceCalendarEvents>({ events: [], scope: 'own' });
	let dayLoading = $state(true),
		eventLoading = $state(true),
		dayError = $state(''),
		eventError = $state(''),
		saveError = $state(''),
		saving = $state(false),
		recovering = $state<'retry' | 'discard' | ''>('');
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
	const manager = $derived($can.has(PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL));
	const writable = $derived(
		(!busy || calendarBusy) &&
			!recovering &&
			!dayLoading &&
			!dayError &&
			!settings.archived &&
			manager
	);
	beforeNavigate(({ cancel, shallow }) => {
		if (shallow || !pendingDays.length || !manager) return;
		cancel();
		toast.error('รอให้ปฏิทินบันทึกเสร็จ หรือลองใหม่ / ยกเลิกรายการค้างก่อนออกจากหน้านี้');
	});
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
	const dayStates = $derived.by(() => {
		const states: Record<string, CalendarDayState> = {};
		for (const cell of buildCalendarMonth(month + '-01')) {
			const pending = pendingDays.find((entry) => entry.day.date === cell.date);
			const counted = pending?.day.counted ?? isCounted(cell.date);
			states[cell.date] = {
				active: cell.inCurrentMonth && !dayLoading && !dayError && counted,
				label:
					!cell.inCurrentMonth || dayLoading || dayError
						? ''
						: counted
							? '✓ นับยอด'
							: '— ไม่นับยอด',
				disabled: !cell.inCurrentMonth || !writable,
				pending: !!pending,
				description: days.find((day) => day.date === cell.date)?.note || undefined
			};
		}
		return states;
	});
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
	async function drainDays(reconcile = false, discard = false) {
		if (saving || disposed || !manager || !pendingDays.length) return;
		const identity = $attendanceIdentity;
		const current = () => !disposed && identity === $attendanceIdentity && manager;
		saving = true;
		recovering = reconcile ? (discard ? 'discard' : 'retry') : '';
		calendarBusy = true;
		onbusy(true);
		if (!reconcile) saveError = '';
		try {
			let rowVersion = settings.rowVersion;
			if (reconcile) {
				const range = attendanceMonthRange(month);
				const [latestSettings, latestDays] = await Promise.all([
					attendanceSettings(term),
					attendanceDays(term, range.from, range.to)
				]);
				if (!current()) return;
				onsettings(latestSettings);
				days = latestDays;
				rowVersion = latestSettings.rowVersion;
				if (discard) pendingDays = [];
				else if (latestSettings.archived)
					throw new Error('ภาคเรียนนี้ถูกเก็บถาวรแล้ว ยกเลิกรายการค้างเพื่อดูข้อมูลล่าสุด');
			}
			while (pendingDays.length && current()) {
				const batch = $state.snapshot(pendingDays).slice(0, 366);
				const values = batch.map((entry) => entry.day);
				const saved = await saveAttendanceDays(term, { days: values, rowVersion });
				if (!current()) return;
				rowVersion = saved.rowVersion;
				onsettings(saved);
				days = [
					...days.filter((day) => !values.some((value) => value.date === day.date)),
					...values
				];
				pendingDays = pendingDays.filter(
					(entry) => !batch.some((sent) => sent.revision === entry.revision)
				);
			}
			if (current()) {
				saveError = '';
				toast.success(discard ? 'ยกเลิกรายการค้างและโหลดข้อมูลล่าสุดแล้ว' : 'บันทึกปฏิทินครบแล้ว');
			}
		} catch (error) {
			if (current()) saveError = error instanceof Error ? error.message : 'บันทึกไม่ได้';
		} finally {
			if (current()) {
				saving = false;
				recovering = '';
				calendarBusy = pendingDays.length > 0;
				onbusy(calendarBusy);
			}
		}
	}
	function setDays(values: AttendanceDay[]) {
		if (!writable || disposed) return;
		if (values.some((value) => new TextEncoder().encode(value.note).length > 500)) {
			saveError = 'หมายเหตุยาวเกินกำหนด กรุณาย่อข้อความ';
			return;
		}
		pendingDays = [
			...pendingDays.filter((entry) => !values.some((value) => value.date === entry.day.date)),
			...values.map((day) => ({ day, revision: ++editRevision }))
		];
		if (!calendarBusy || saving || !saveError) {
			saveError = '';
			void drainDays();
		}
	}
	function toggleDay(value: string) {
		if (value.slice(0, 7) !== month || !writable) return;
		void setDays([
			{
				date: value,
				counted: !(
					pendingDays.find((entry) => entry.day.date === value)?.day.counted ?? isCounted(value)
				),
				note:
					note ||
					pendingDays.find((entry) => entry.day.date === value)?.day.note ||
					days.find((day) => day.date === value)?.note ||
					''
			}
		]);
	}
	function applyRange(counted: boolean) {
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
			setDays(values);
		} catch (error) {
			saveError = error instanceof Error ? error.message : 'ช่วงวันที่ไม่ถูกต้อง';
		}
	}
</script>

<Card.Root>
	<Card.Header
		><Card.Title><h2>ปฏิทินวันประมวลผล</h2></Card.Title><Card.Description
			>กดช่องวันเพื่อสลับการนับยอดทันที กดแถบกิจกรรมเพื่อดูรายละเอียดประกอบการตัดสินใจ</Card.Description
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
				: 'กิจกรรมจากปฏิทินที่คุณมีสิทธิ์เห็น'} · ช่องสีเข้ม ✓ นับยอด · ช่องสีอ่อน — ไม่นับยอด
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
		<p role="status" class="min-h-5 text-sm text-muted-foreground">
			{pendingDays.length
				? `${saving ? 'กำลังบันทึก' : 'ยังไม่บันทึก'} ${pendingDays.length} วัน · ${recovering ? 'กำลังโหลดข้อมูลล่าสุด' : 'กดเปลี่ยนวันต่อได้'}`
				: 'บันทึกอัตโนมัติเมื่อเปลี่ยนวัน'}
		</p>
		<div aria-busy={dayLoading || eventLoading}>
			<CalendarMonthGrid
				monthDate={month + '-01'}
				events={calendar.events}
				{dayStates}
				hideMobileTimes
				onselect={toggleDay}
				oneventselect={(_, value) => {
					selectedDate = value;
					dayDialogOpen = true;
				}}
			/>
		</div>
		{#if saveError}<Alert.Root variant="destructive"
				><Alert.Title>บันทึกวันประมวลผลไม่ได้</Alert.Title><Alert.Description
					>{saveError}</Alert.Description
				>
				{#if pendingDays.length}<p class="text-sm">
						ค่าที่เลือกยังรอบันทึก ลองใหม่จะโหลดข้อมูลล่าสุดก่อนบันทึกเฉพาะวันที่เลือก
					</p>
					<div class="flex flex-wrap gap-2">
						<LoadingButton
							type="button"
							loading={recovering === 'retry'}
							disabled={saving}
							onclick={() => void drainDays(true)}>โหลดล่าสุดแล้วลองใหม่</LoadingButton
						>
						<LoadingButton
							type="button"
							variant="outline"
							loading={recovering === 'discard'}
							disabled={saving}
							onclick={() => void drainDays(true, true)}>ยกเลิกรายการค้าง</LoadingButton
						>
					</div>
				{/if}
			</Alert.Root>{/if}
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
			<Button
				type="button"
				variant="outline"
				disabled={!writable || busy || saving}
				onclick={() => applyRange(true)}>นับช่วงนี้</Button
			><Button
				type="button"
				variant="outline"
				disabled={!writable || busy || saving}
				onclick={() => applyRange(false)}>วันหยุด / ไม่นับช่วงนี้</Button
			>
		</div>
	</Card.Content>
</Card.Root>

<CalendarDayTimelineDialog bind:open={dayDialogOpen} date={selectedDate} events={selectedEvents} />
