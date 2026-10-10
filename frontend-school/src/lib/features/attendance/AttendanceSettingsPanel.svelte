<script lang="ts">
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { onDestroy, untrack } from 'svelte';
	import { SvelteDate } from 'svelte/reactivity';
	import { toast } from 'svelte-sonner';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import {
		attendanceDays,
		saveAttendanceDays,
		saveAttendanceSettings,
		type AttendanceSettings,
		type AttendanceDay
	} from '#lib/api/attendance.js';
	let {
		term,
		date,
		initial
	}: {
		term: string;
		date: string;
		initial: [AttendanceSettings, AttendanceDay[]];
	} = $props();
	let settings = $state(untrack(() => initial[0])),
		days = $state(untrack(() => initial[1])),
		configuration = $state(untrack(() => $state.snapshot(initial[0].configuration))),
		month = $state(untrack(() => date.slice(0, 7))),
		note = $state(''),
		busy = $state(false),
		pendingAction = $state(''),
		calendarError = $state(''),
		calendarLoading = $state(false),
		digests = $state(
			untrack(() => initial[0].configuration.digestTimes.map((t) => t.slice(0, 5)).join(', '))
		);
	let rangeStart = $state(untrack(() => date)),
		rangeEnd = $state(untrack(() => date));
	const manager = $derived($can.has(PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL));
	const cells = $derived.by(() => {
		const [year, m] = month.split('-').map(Number);
		if (!/^\d{4}-\d{2}$/.test(month) || m < 1 || m > 12) return [];
		const start = new Date(Date.UTC(year, m - 1, 1));
		const total = new Date(Date.UTC(year, m, 0)).getUTCDate();
		return [
			...Array((start.getUTCDay() + 6) % 7).fill(null),
			...Array.from({ length: total }, (_, i) => `${month}-${String(i + 1).padStart(2, '0')}`)
		] as (string | null)[];
	});
	const calendarRequest = new LatestRequest();
	onDestroy(() => calendarRequest.abort());
	const weekdays = ['จ', 'อ', 'พ', 'พฤ', 'ศ', 'ส', 'อา'];
	function isCounted(value: string) {
		return (
			days.find((d) => d.date === value)?.counted ??
			settings.configuration.weekdays.includes(
				((new Date(value + 'T00:00:00Z').getUTCDay() + 6) % 7) + 1
			)
		);
	}
	async function run(name: string, action: () => Promise<void>) {
		if (busy || settings.archived) return;
		busy = true;
		pendingAction = name;
		try {
			await action();
			toast.success('บันทึกแล้ว');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่ได้');
		} finally {
			busy = false;
			pendingAction = '';
		}
	}
	async function saveConfig() {
		await run('settings', async () => {
			if (configuration.lateAfter.length === 5) configuration.lateAfter += ':00';
			configuration.digestTimes = digests
				.split(',')
				.map((t) => t.trim())
				.filter(Boolean)
				.map((t) => (t.length === 5 ? t + ':00' : t));
			settings = await saveAttendanceSettings(term, {
				configuration,
				rowVersion: settings.rowVersion
			});
			configuration = $state.snapshot(settings.configuration);
		});
	}
	async function loadMonth() {
		const t = calendarRequest.begin();
		calendarError = '';
		calendarLoading = true;
		try {
			if (!/^\d{4}-\d{2}$/.test(month)) throw new Error('เลือกเดือนที่ต้องการ');
			const [year, m] = month.split('-').map(Number);
			const end = new Date(Date.UTC(year, m, 0)).toISOString().slice(0, 10);
			const result = await attendanceDays(term, month + '-01', end, { signal: t.signal });
			if (calendarRequest.isCurrent(t.revision)) days = result;
		} catch (e) {
			if (calendarRequest.isCurrent(t.revision))
				calendarError = e instanceof Error ? e.message : 'โหลดปฏิทินไม่ได้';
		} finally {
			if (calendarRequest.isCurrent(t.revision)) calendarLoading = false;
		}
	}
	async function setDays(values: AttendanceDay[]) {
		if (calendarLoading || calendarError) return;
		await run('days', async () => {
			settings = await saveAttendanceDays(term, { days: values, rowVersion: settings.rowVersion });
			days = [...days.filter((d) => !values.some((v) => v.date === d.date)), ...values];
		});
	}
	function dateRange(start: string, end: string) {
		if (!start || !end || start > end) throw new Error('เลือกช่วงวันที่ให้ถูกต้อง');
		const values: string[] = [];
		const current = new SvelteDate(start + 'T00:00:00Z');
		const stop = new Date(end + 'T00:00:00Z');
		while (current <= stop && values.length <= 366) {
			values.push(current.toISOString().slice(0, 10));
			current.setUTCDate(current.getUTCDate() + 1);
		}
		if (values.length > 366) throw new Error('เลือกครั้งละไม่เกิน 366 วัน');
		return values;
	}
	async function applyRange(value: boolean) {
		try {
			await setDays(
				dateRange(rangeStart, rangeEnd).map((d) => ({ date: d, counted: value, note }))
			);
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'ช่วงวันที่ไม่ถูกต้อง');
		}
	}
</script>

{#if !manager}<p role="alert">ต้องมีสิทธิ์ตั้งค่าระบบเช็คชื่อทั้งโรงเรียน</p>{:else}
	<section class="space-y-4 rounded-xl border bg-card p-4 sm:p-5">
		<fieldset disabled={busy || settings.archived} class="space-y-4">
			<h2 class="text-lg font-semibold">การประมวลผลและแจ้งเตือน</h2>
			<label class="flex gap-2"
				><Checkbox
					bind:checked={configuration.enabled}
					disabled={settings.archived}
				/>เปิดส่งสรุปครูตามเวลา</label
			>
			<p class="text-sm text-muted-foreground">
				เลือกวันเพื่อนับในยอดสรุปและส่งสรุปครู วันที่ไม่ได้เลือกยังบันทึกและแจ้งเตือนรายคนได้
			</p>
			<div class="flex flex-wrap gap-3">
				{#each weekdays as day, i (i)}
					<label class="flex items-center gap-2"
						><Checkbox
							checked={configuration.weekdays.includes(i + 1)}
							onCheckedChange={(checked) => {
								configuration.weekdays = checked
									? [...configuration.weekdays, i + 1]
									: configuration.weekdays.filter((day) => day !== i + 1);
							}}
						/>
						{day}</label
					>{/each}
			</div>
			<div class="grid gap-4 md:grid-cols-3">
				<label>เข้าสายหลังเวลา<Input type="time" bind:value={configuration.lateAfter} /></label
				><label>ส่งสรุปครู (เช่น 09:00, 16:00)<Input bind:value={digests} /></label><label
					>เก็บภาพหลักฐาน (วัน)<Input
						type="number"
						min={1}
						max={365}
						bind:value={configuration.evidenceDays}
					/></label
				>
			</div>
			<label class="flex gap-2"
				><Checkbox
					bind:checked={configuration.activityCountsAsPresent}
				/>นับกิจกรรมร่วมกับมาในการคำนวณร้อยละ</label
			>
			<details>
				<summary>ความเข้มงวดการจับคู่ใบหน้า</summary>
				<div class="grid gap-3 md:grid-cols-2">
					<label
						>ระยะสูงสุด (ค่าน้อยเข้มงวดขึ้น)<Input
							type="number"
							step={0.01}
							min={0.2}
							max={0.6}
							bind:value={configuration.faceDistance}
						/></label
					><label
						>ส่วนต่างจากคนที่คล้ายกัน<Input
							type="number"
							step={0.01}
							min={0.05}
							max={0.3}
							bind:value={configuration.faceMargin}
						/></label
					>
				</div>
			</details>
			<LoadingButton
				loading={pendingAction === 'settings'}
				onclick={saveConfig}
				disabled={busy || settings.archived}>บันทึกตั้งค่า</LoadingButton
			>
		</fieldset>
	</section>
	<section class="space-y-4 rounded-xl border bg-card p-4 sm:p-5">
		<h2 class="text-lg font-semibold">ปฏิทินวันประมวลผล</h2>
		<div class="flex flex-wrap items-end gap-3">
			<label
				>เดือน<Input type="month" bind:value={month} onchange={loadMonth} disabled={busy} /></label
			><label>หมายเหตุวันหยุด / กิจกรรม<Input bind:value={note} /></label>
		</div>
		{#if calendarError}<PageState
				variant="error"
				title="โหลดปฏิทินไม่ได้"
				description={calendarError}
				actionLabel="ลองโหลดปฏิทินใหม่"
				onaction={loadMonth}
			/>{/if}
		{#if calendarLoading}<p role="status">กำลังโหลดปฏิทิน…</p>{/if}
		<div aria-busy={calendarLoading} class="grid max-w-2xl grid-cols-7 gap-1 sm:gap-2">
			{#each weekdays as day (day)}
				<span class="text-center">{day}</span>{/each}{#each cells as value, index (index)}
				{#if value}<Button
						class="min-w-0 px-1 sm:px-3"
						aria-pressed={isCounted(value)}
						variant={isCounted(value) ? 'default' : 'outline'}
						disabled={busy || calendarLoading || !!calendarError || settings.archived}
						onclick={() => setDays([{ date: value, counted: !isCounted(value), note }])}
						title={days.find((d) => d.date === value)?.note ?? ''}
						>{Number(value.slice(-2))} {isCounted(value) ? '✓' : '—'}</Button
					>{:else}<span></span>{/if}{/each}
		</div>
		<div class="flex flex-wrap items-end gap-3">
			<label>จาก<DatePicker bind:value={rangeStart} /></label><label
				>ถึง<DatePicker bind:value={rangeEnd} /></label
			><Button
				variant="outline"
				disabled={busy || calendarLoading || !!calendarError || settings.archived}
				onclick={() => applyRange(true)}>นับช่วงนี้</Button
			><Button
				variant="outline"
				disabled={busy || calendarLoading || !!calendarError || settings.archived}
				onclick={() => applyRange(false)}>วันหยุด / ไม่นับช่วงนี้</Button
			>
		</div>
	</section>
{/if}
