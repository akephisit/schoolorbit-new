<script lang="ts">
	import { untrack } from 'svelte';
	import { SvelteDate } from 'svelte/reactivity';
	import { toast } from 'svelte-sonner';
	import AttendanceSelect from './AttendanceSelect.svelte';
	import AttendanceMultiSelect from './AttendanceMultiSelect.svelte';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import {
		saveAttendanceAudience,
		createAttendanceSpecial,
		saveAttendanceDevice,
		type AttendanceOptions,
		type SpecialAttendanceGroup
	} from '#lib/api/attendance.js';
	let {
		term,
		date,
		initial,
		archived
	}: { term: string; date: string; initial: AttendanceOptions; archived: boolean } = $props();
	let options = $state(untrack(() => initial)),
		busy = $state(false),
		pendingAction = $state('');
	let audienceId = $state<string | null>(null),
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
	const weekdays = ['จ', 'อ', 'พ', 'พฤ', 'ศ', 'ส', 'อา'];
	const rooms = $derived(
		Array.from(
			new Map(
				options.students
					.filter((s) => s.homeroomId)
					.map((s) => [s.homeroomId!, s.homeroomName ?? 'ห้องเรียน'])
			).entries()
		)
	);
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
	async function run(name: string, action: () => Promise<void>) {
		if (busy || archived) return;
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
	async function createAudience() {
		await run('audience', async () => {
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
		await run('special', async () => {
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
		await run('device', async () => {
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
		await run('device-toggle', async () => {
			const saved = await saveAttendanceDevice(id, {
				name: device.name,
				operatorId: device.operatorId,
				enabled: !device.enabled
			});
			options = { ...options, devices: options.devices.map((d) => (d.id === id ? saved : d)) };
		});
	}
</script>

<fieldset disabled={busy || archived} class="min-w-0 space-y-4">
	<section class="space-y-4 rounded-xl border bg-card p-4 sm:p-5">
		<h2 class="text-lg font-semibold">กลุ่มนักเรียนที่ใช้ซ้ำ</h2>
		<label>ชื่อกลุ่ม<Input bind:value={audienceName} /></label><label class="block"
			>เลือกสมาชิก<AttendanceMultiSelect
				label="สมาชิกกลุ่ม"
				bind:value={audienceStudents}
				options={options.students.map((s) => ({
					value: s.id,
					label: `${s.homeroomName ?? '-'} · ${s.name}`
				}))}
				disabled={busy || archived}
			/></label
		><LoadingButton
			loading={pendingAction === 'audience'}
			onclick={createAudience}
			disabled={busy || archived}>{audienceId ? 'บันทึกกลุ่ม' : 'เพิ่มกลุ่ม'}</LoadingButton
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
	<section class="space-y-4 rounded-xl border bg-card p-4 sm:p-5">
		<h2 class="text-lg font-semibold">เพิ่มรอบเช็คชื่อพิเศษ</h2>
		<label>ชื่อการเช็คชื่อ<Input bind:value={title} placeholder="เช่น ชุมนุม / ทัศนศึกษา" /></label>
		<div class="grid gap-3 md:grid-cols-4">
			<label>เริ่มวันที่<DatePicker bind:value={specialStart} /></label><label
				>ถึงวันที่<DatePicker bind:value={specialEnd} /></label
			><label>เวลาเริ่ม<Input type="time" bind:value={startTime} /></label><label
				>เวลาจบ<Input type="time" bind:value={endTime} /></label
			>
		</div>
		<div class="flex flex-wrap gap-3">
			{#each weekdays as day, i (i)}
				<label class="flex items-center gap-2"
					><Checkbox
						checked={repeatDays.includes(i + 1)}
						onCheckedChange={(checked) => {
							repeatDays = checked
								? [...repeatDays, i + 1]
								: repeatDays.filter((day) => day !== i + 1);
						}}
					/>
					{day}</label
				>{/each}
		</div>
		<div class="flex flex-wrap gap-4">
			<label class="flex items-center gap-2"
				><Checkbox bind:checked={counted} /> นับยอดกิจกรรมนี้</label
			><label><Checkbox bind:checked={notify} /> แจ้งนักเรียนและผู้ปกครอง</label>
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
					><Checkbox bind:checked={group.useHomeroomAdvisors} /> ให้ครูประจำชั้นรับผิดชอบห้องที่เลือก</label
				>
				<div class="grid gap-3 md:grid-cols-2">
					<label
						>ห้องเรียน<AttendanceMultiSelect
							label="ห้องเรียน"
							bind:value={group.homeroomIds}
							options={rooms.map(([value, label]) => ({ value, label }))}
							disabled={busy || archived}
						/></label
					><label
						>ครูที่เลือกเพิ่ม<AttendanceMultiSelect
							label="ครูที่เลือกเพิ่ม"
							bind:value={group.teacherIds}
							options={options.teachers.map((t) => ({ value: t.id, label: t.name }))}
							disabled={busy || archived}
						/></label
					><label
						>กลุ่มที่ตั้งไว้<AttendanceMultiSelect
							label="กลุ่มที่ตั้งไว้"
							bind:value={group.audienceGroupIds}
							options={options.audiences.map((a) => ({ value: a.id, label: a.name }))}
							disabled={busy || archived}
						/></label
					><label
						>นักเรียนรายคน<AttendanceMultiSelect
							label="นักเรียนรายคน"
							bind:value={group.studentIds}
							options={options.students.map((s) => ({ value: s.id, label: s.name }))}
							disabled={busy || archived}
						/></label
					>
				</div>
			</div>{/each}
		<div class="flex flex-wrap gap-3">
			<Button
				variant="outline"
				onclick={() => {
					groups = [...groups, { ...newGroup(), name: `กลุ่ม ${groups.length + 1}` }];
				}}>เพิ่มกลุ่มในรอบ</Button
			><LoadingButton
				loading={pendingAction === 'special'}
				onclick={createSpecial}
				disabled={busy || archived}>สร้างรอบพิเศษ</LoadingButton
			>
		</div>
		<ul>
			{#each options.specials as s (s.id)}
				<li>
					{s.definition.title} · {s.definition.dates.length} วัน / {s.definition.groups.length} กลุ่ม
				</li>{/each}
		</ul>
	</section>
	<section class="space-y-4 rounded-xl border bg-card p-4 sm:p-5">
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
				disabled={busy || archived}
			/><LoadingButton
				loading={pendingAction === 'device'}
				onclick={createDevice}
				disabled={busy || archived || !operator}>เพิ่มเครื่อง</LoadingButton
			>
		</div>
		{#each options.devices as d (d.id)}
			<div class="flex flex-wrap items-center gap-3">
				<span>{d.name} · {options.teachers.find((t) => t.id === d.operatorId)?.name}</span><Button
					variant="outline"
					disabled={busy}
					onclick={() => toggleDevice(d.id)}>{d.enabled ? 'ปิดเครื่อง' : 'เปิดเครื่อง'}</Button
				>
			</div>{/each}
	</section>
</fieldset>
