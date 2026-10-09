<script lang="ts">
	import AttendanceSelect from './AttendanceSelect.svelte';
	import AttendanceMultiSelect from './AttendanceMultiSelect.svelte';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { untrack } from 'svelte';
	import { SvelteDate } from 'svelte/reactivity';
	import { toast } from 'svelte-sonner';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import {
		attendanceDays,
		saveAttendanceDays,
		saveAttendanceSettings,
		saveAttendanceAudience,
		createAttendanceSpecial,
		saveAttendanceDevice,
		type AttendanceSettings,
		type AttendanceOptions,
		type AttendanceDay,
		type SpecialAttendanceGroup
	} from '#lib/api/attendance.js';
	let {
		term,
		date,
		initial
	}: {
		term: string;
		date: string;
		initial: [AttendanceSettings, AttendanceOptions, AttendanceDay[]];
	} = $props();
	let settings = $state(untrack(() => initial[0])),
		options = $state(untrack(() => initial[1])),
		days = $state(untrack(() => initial[2])),
		configuration = $state(untrack(() => structuredClone(initial[0].configuration))),
		month = $state(untrack(() => date.slice(0, 7))),
		note = $state(''),
		busy = $state(false),
		digests = $state(
			untrack(() => initial[0].configuration.digestTimes.map((t) => t.slice(0, 5)).join(', '))
		);
	let rangeStart = $state(untrack(() => date)),
		rangeEnd = $state(untrack(() => date)),
		audienceId = $state<string | null>(null),
		audienceVersion = $state(0),
		audienceName = $state(''),
		audienceStudents = $state<string[]>([]),
		title = $state(''),
		specialStart = $state(untrack(() => date)),
		specialEnd = $state(untrack(() => date)),
		repeatDays = $state<number[]>([1, 2, 3, 4, 5]),
		startTime = $state('09:00'),
		endTime = $state('10:00'),
		counted = $state(true),
		notify = $state(true),
		groups = $state<SpecialAttendanceGroup[]>([newGroup()]),
		deviceName = $state('เครื่องหน้าโรงเรียน'),
		operator = $state('');
	const manager = $derived($can.has(PERMISSIONS.ATTENDANCE_MANAGE_SCHOOL));
	const rooms = $derived(
		Array.from(
			new Map(
				options.students
					.filter((s) => s.homeroomId)
					.map((s) => [s.homeroomId!, s.homeroomName ?? 'ห้องเรียน'])
			).entries()
		)
	);
	const cells = $derived.by(() => {
		const [year, m] = month.split('-').map(Number);
		const start = new Date(Date.UTC(year, m - 1, 1));
		const total = new Date(Date.UTC(year, m, 0)).getUTCDate();
		return [
			...Array((start.getUTCDay() + 6) % 7).fill(null),
			...Array.from({ length: total }, (_, i) => `${month}-${String(i + 1).padStart(2, '0')}`)
		] as (string | null)[];
	});
	const weekdays = ['จ', 'อ', 'พ', 'พฤ', 'ศ', 'ส', 'อา'];
	function newGroup(): SpecialAttendanceGroup {
		return {
			name: 'กลุ่ม 1',
			teacherIds: [],
			homeroomIds: [],
			audienceGroupIds: [],
			studentIds: [],
			useHomeroomAdvisors: true
		};
	}
	function isCounted(value: string) {
		return (
			days.find((d) => d.date === value)?.counted ??
			configuration.weekdays.includes(((new Date(value + 'T00:00:00Z').getUTCDay() + 6) % 7) + 1)
		);
	}
	async function run(action: () => Promise<void>) {
		busy = true;
		try {
			await action();
			toast.success('บันทึกแล้ว');
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'บันทึกไม่ได้');
		} finally {
			busy = false;
		}
	}
	async function saveConfig() {
		await run(async () => {
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
			configuration = structuredClone(settings.configuration);
		});
	}
	async function loadMonth() {
		busy = true;
		try {
			const [year, m] = month.split('-').map(Number);
			const end = new Date(Date.UTC(year, m, 0)).toISOString().slice(0, 10);
			days = await attendanceDays(term, month + '-01', end);
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดปฏิทินไม่ได้');
		} finally {
			busy = false;
		}
	}
	async function setDays(values: AttendanceDay[]) {
		await run(async () => {
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
	async function createAudience() {
		await run(async () => {
			const result = await saveAttendanceAudience(term, {
				id: audienceId,
				name: audienceName,
				studentIds: audienceStudents,
				rowVersion: audienceVersion
			});
			options = {
				...options,
				audiences: [...options.audiences.filter((a) => a.id !== result.id), result]
			};
			audienceId = null;
			audienceVersion = 0;
			audienceName = '';
			audienceStudents = [];
		});
	}
	async function createSpecial() {
		await run(async () => {
			const dates = dateRange(specialStart, specialEnd).filter(
				(d) =>
					specialStart === specialEnd ||
					repeatDays.includes(((new Date(d + 'T00:00:00Z').getUTCDay() + 6) % 7) + 1)
			);
			const result = await createAttendanceSpecial(term, {
				title,
				dates,
				startTime: startTime + ':00',
				endTime: endTime + ':00',
				counted,
				notify,
				groups
			});
			options = { ...options, specials: [...options.specials, result] };
			title = '';
		});
	}
	async function createDevice() {
		await run(async () => {
			const device = await saveAttendanceDevice(crypto.randomUUID(), {
				name: deviceName,
				enabled: true,
				operatorId: operator
			});
			options = { ...options, devices: [...options.devices, device] };
		});
	}
	async function toggleDevice(id: string) {
		const device = options.devices.find((d) => d.id === id);
		if (!device) return;
		await run(async () => {
			const saved = await saveAttendanceDevice(id, {
				name: device.name,
				operatorId: device.operatorId,
				enabled: !device.enabled
			});
			options = { ...options, devices: options.devices.map((d) => (d.id === id ? saved : d)) };
		});
	}
</script>

<Button variant="outline" href={`/staff/attendance?academicTermId=${term}`}>กลับหน้าเช็คชื่อ</Button
>
{#if !manager}<p role="alert">ต้องมีสิทธิ์ตั้งค่าระบบเช็คชื่อทั้งโรงเรียน</p>{:else}
	<section class="space-y-4 rounded-xl border p-5">
		<h2 class="text-lg font-semibold">การประมวลผลและแจ้งเตือน</h2>
		<label class="flex gap-2"
			><input
				type="checkbox"
				bind:checked={configuration.enabled}
				disabled={settings.archived}
			/>เปิดส่งสรุปครูตามเวลา</label
		>
		<p class="text-sm text-muted-foreground">
			เลือกวันเพื่อนับในยอดสรุปและส่งสรุปครู วันที่ไม่ได้เลือกยังบันทึกและแจ้งเตือนรายคนได้
		</p>
		<div class="flex gap-3">
			{#each weekdays as day, i (i)}
				<label
					><input type="checkbox" value={i + 1} bind:group={configuration.weekdays} /> {day}</label
				>{/each}
		</div>
		<div class="grid gap-4 md:grid-cols-3">
			<label>เข้าสายหลังเวลา<Input type="time" bind:value={configuration.lateAfter} /></label><label
				>ส่งสรุปครู (เช่น 09:00, 16:00)<Input bind:value={digests} /></label
			><label
				>เก็บภาพหลักฐาน (วัน)<Input
					type="number"
					min={1}
					max={365}
					bind:value={configuration.evidenceDays}
				/></label
			>
		</div>
		<label class="flex gap-2"
			><input
				type="checkbox"
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
		<Button onclick={saveConfig} disabled={busy || settings.archived}>บันทึกตั้งค่า</Button>
	</section>
	<section class="space-y-4 rounded-xl border p-5">
		<h2 class="text-lg font-semibold">ปฏิทินวันประมวลผล</h2>
		<div class="flex flex-wrap items-end gap-3">
			<label>เดือน<Input type="month" bind:value={month} onchange={loadMonth} /></label><label
				>หมายเหตุวันหยุด / กิจกรรม<Input bind:value={note} /></label
			>
		</div>
		<div class="grid max-w-2xl grid-cols-7 gap-2">
			{#each weekdays as day (day)}
				<span class="text-center">{day}</span>{/each}{#each cells as value, index (index)}
				{#if value}<Button
						variant={isCounted(value) ? 'default' : 'outline'}
						disabled={busy || settings.archived}
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
				disabled={busy || settings.archived}
				onclick={() => applyRange(true)}>นับช่วงนี้</Button
			><Button
				variant="outline"
				disabled={busy || settings.archived}
				onclick={() => applyRange(false)}>วันหยุด / ไม่นับช่วงนี้</Button
			>
		</div>
	</section>
	<section class="space-y-4 rounded-xl border p-5">
		<h2 class="text-lg font-semibold">กลุ่มนักเรียนที่ใช้ซ้ำ</h2>
		<label>ชื่อกลุ่ม<Input bind:value={audienceName} /></label><label class="block"
			>เลือกสมาชิก<AttendanceMultiSelect
				label="สมาชิกกลุ่ม"
				bind:value={audienceStudents}
				options={options.students.map((s) => ({
					value: s.id,
					label: `${s.homeroomName ?? '-'} · ${s.name}`
				}))}
				disabled={busy || settings.archived}
			/></label
		><Button onclick={createAudience} disabled={busy || settings.archived}
			>{audienceId ? 'บันทึกกลุ่ม' : 'เพิ่มกลุ่ม'}</Button
		>
		<ul>
			{#each options.audiences as group (group.id)}
				<li>
					{group.name} · {group.studentIds.length} คน <Button
						variant="link"
						onclick={() => {
							audienceId = group.id;
							audienceVersion = group.rowVersion;
							audienceName = group.name;
							audienceStudents = [...group.studentIds];
						}}>แก้ไขกลุ่ม</Button
					>
				</li>{/each}
		</ul>
	</section>
	<section class="space-y-4 rounded-xl border p-5">
		<h2 class="text-lg font-semibold">เพิ่มรอบเช็คชื่อพิเศษ</h2>
		<label>ชื่อการเช็คชื่อ<Input bind:value={title} placeholder="เช่น ชุมนุม / ทัศนศึกษา" /></label>
		<div class="grid gap-3 md:grid-cols-4">
			<label>เริ่มวันที่<DatePicker bind:value={specialStart} /></label><label
				>ถึงวันที่<DatePicker bind:value={specialEnd} /></label
			><label>เวลาเริ่ม<Input type="time" bind:value={startTime} /></label><label
				>เวลาจบ<Input type="time" bind:value={endTime} /></label
			>
		</div>
		<div class="flex gap-3">
			{#each weekdays as day, i (i)}
				<label><input type="checkbox" value={i + 1} bind:group={repeatDays} /> {day}</label>{/each}
		</div>
		<div class="flex gap-4">
			<label><input type="checkbox" bind:checked={counted} /> นับยอดกิจกรรมนี้</label><label
				><input type="checkbox" bind:checked={notify} /> แจ้งนักเรียนและผู้ปกครอง</label
			>
		</div>
		<p class="text-sm text-muted-foreground">
			แต่ละวันเป็นหนึ่งรอบ นักเรียนอยู่ได้เพียงกลุ่มเดียวภายในรอบเดียวกัน ระบบตรวจกลุ่มซ้ำก่อนบันทึก
		</p>
		{#each groups as group, i (i)}
			<div class="space-y-3 rounded-lg bg-muted p-4">
				<div class="flex gap-2">
					<Input aria-label="ชื่อกลุ่มในรอบ" bind:value={group.name} /><Button
						variant="outline"
						onclick={() => {
							groups = groups.filter((_, index) => index !== i);
						}}
						disabled={groups.length === 1}>เอากลุ่มออก</Button
					>
				</div>
				<label
					><input type="checkbox" bind:checked={group.useHomeroomAdvisors} /> ให้ครูประจำชั้นรับผิดชอบห้องที่เลือก</label
				>
				<div class="grid gap-3 md:grid-cols-2">
					<label
						>ห้องเรียน<AttendanceMultiSelect
							label="ห้องเรียน"
							bind:value={group.homeroomIds}
							options={rooms.map(([value, label]) => ({ value, label }))}
							disabled={busy || settings.archived}
						/></label
					><label
						>ครูที่เลือกเพิ่ม<AttendanceMultiSelect
							label="ครูที่เลือกเพิ่ม"
							bind:value={group.teacherIds}
							options={options.teachers.map((t) => ({ value: t.id, label: t.name }))}
							disabled={busy || settings.archived}
						/></label
					><label
						>กลุ่มที่ตั้งไว้<AttendanceMultiSelect
							label="กลุ่มที่ตั้งไว้"
							bind:value={group.audienceGroupIds}
							options={options.audiences.map((a) => ({ value: a.id, label: a.name }))}
							disabled={busy || settings.archived}
						/></label
					><label
						>นักเรียนรายคน<AttendanceMultiSelect
							label="นักเรียนรายคน"
							bind:value={group.studentIds}
							options={options.students.map((s) => ({ value: s.id, label: s.name }))}
							disabled={busy || settings.archived}
						/></label
					>
				</div>
			</div>{/each}
		<div class="flex gap-3">
			<Button
				variant="outline"
				onclick={() => {
					groups = [...groups, { ...newGroup(), name: `กลุ่ม ${groups.length + 1}` }];
				}}>เพิ่มกลุ่มในรอบ</Button
			><Button onclick={createSpecial} disabled={busy || settings.archived}>สร้างรอบพิเศษ</Button>
		</div>
		<ul>
			{#each options.specials as s (s.id)}
				<li>
					{s.definition.title} · {s.definition.dates.length} วัน / {s.definition.groups.length} กลุ่ม
				</li>{/each}
		</ul>
	</section>
	<section class="space-y-4 rounded-xl border p-5">
		<h2 class="text-lg font-semibold">เครื่องเว็บแคม</h2>
		<p class="text-sm text-muted-foreground">
			เครื่องสแกนต้องลงชื่อเข้าใช้บัญชีครูที่มอบหมายและมีสิทธิ์ใช้เครื่องสแกน
		</p>
		<div class="flex flex-wrap gap-3">
			<Input aria-label="ชื่อเครื่อง" bind:value={deviceName} class="max-w-xs" /><AttendanceSelect
				label="ผู้ใช้เครื่อง"
				placeholder="เลือกผู้ใช้เครื่อง"
				bind:value={operator}
				options={options.teachers.map((t) => ({ value: t.id, label: t.name }))}
				disabled={busy || settings.archived}
			/><Button onclick={createDevice} disabled={busy || !operator}>เพิ่มเครื่อง</Button>
		</div>
		{#each options.devices as d (d.id)}
			<div class="flex items-center gap-3">
				<span>{d.name} · {options.teachers.find((t) => t.id === d.operatorId)?.name}</span><Button
					variant="outline"
					disabled={busy}
					onclick={() => toggleDevice(d.id)}>{d.enabled ? 'ปิดเครื่อง' : 'เปิดเครื่อง'}</Button
				>
			</div>{/each}
	</section>
{/if}
