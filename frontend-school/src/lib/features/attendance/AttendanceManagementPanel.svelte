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
	import { Label } from '#lib/components/ui/label/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as Alert from '#lib/components/ui/alert/index.js';
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
		pendingAction = $state(''),
		errors = $state<Record<string, string>>({}),
		fieldErrors = $state<Record<string, string>>({});
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
		errors[name.startsWith('device') ? 'device' : name] = '';
		try {
			await action();
			toast.success('บันทึกแล้ว');
		} catch (e) {
			const message = e instanceof Error ? e.message : 'บันทึกไม่ได้';
			errors[name.startsWith('device') ? 'device' : name] = message;
			toast.error(message);
		} finally {
			busy = false;
			pendingAction = '';
		}
	}
	function dateRange(start: string, end: string) {
		if (
			!start ||
			!end ||
			!Number.isFinite(Date.parse(start + 'T00:00:00Z')) ||
			!Number.isFinite(Date.parse(end + 'T00:00:00Z')) ||
			start > end
		)
			throw new Error('เลือกช่วงวันที่ให้ถูกต้อง');
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
			fieldErrors.audienceName = audienceName.trim() ? '' : 'ระบุชื่อกลุ่ม';
			fieldErrors.audienceStudents = audienceStudents.length ? '' : 'เลือกสมาชิกอย่างน้อยหนึ่งคน';
			if (fieldErrors.audienceName || fieldErrors.audienceStudents)
				throw new Error('ตรวจชื่อกลุ่มและสมาชิกที่ระบุไว้');
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
			fieldErrors.title = title.trim() ? '' : 'ระบุชื่อการเช็คชื่อ';
			fieldErrors.specialDates = '';
			fieldErrors.specialTimes = '';
			fieldErrors.groups = groups.some((group) => !group.name.trim())
				? 'ระบุชื่อทุกกลุ่มในรอบ'
				: '';
			if (!startTime || !endTime || endTime <= startTime)
				fieldErrors.specialTimes = 'เวลาจบต้องหลังเวลาเริ่ม';
			if (fieldErrors.title || fieldErrors.specialTimes || fieldErrors.groups)
				throw new Error('ตรวจข้อมูลรอบพิเศษที่ระบุไว้');
			let dates: string[];
			try {
				dates = dateRange(specialStart, specialEnd).filter(
					(d) =>
						specialStart === specialEnd ||
						repeatDays.includes(((new Date(d + 'T00:00:00Z').getUTCDay() + 6) % 7) + 1)
				);
			} catch (error) {
				fieldErrors.specialDates =
					error instanceof Error ? error.message : 'เลือกช่วงวันที่ให้ถูกต้อง';
				throw error;
			}
			if (!dates.length) {
				fieldErrors.specialDates = 'เลือกวันในช่วงวันที่อย่างน้อยหนึ่งวัน';
				throw new Error(fieldErrors.specialDates);
			}
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
			fieldErrors.deviceName = deviceName.trim() ? '' : 'ระบุชื่อเครื่อง';
			fieldErrors.operator = operator ? '' : 'เลือกผู้ใช้เครื่อง';
			if (fieldErrors.deviceName || fieldErrors.operator)
				throw new Error('ตรวจชื่อเครื่องและผู้ใช้ที่ระบุไว้');
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
		await run(`device-toggle:${id}`, async () => {
			const saved = await saveAttendanceDevice(id, {
				name: device.name,
				operatorId: device.operatorId,
				enabled: !device.enabled
			});
			options = { ...options, devices: options.devices.map((d) => (d.id === id ? saved : d)) };
		});
	}
</script>

<fieldset disabled={busy || archived} class="min-w-0 space-y-6">
	<Card.Root>
		<Card.Header
			><Card.Title><h2>กลุ่มนักเรียนที่ใช้ซ้ำ</h2></Card.Title><Card.Description
				>รวมสมาชิกไว้เป็นกลุ่มสำหรับเลือกใช้ในรอบเช็คชื่อพิเศษ</Card.Description
			></Card.Header
		>
		<Card.Content class="space-y-6">
			<form
				onsubmit={(event) => {
					event.preventDefault();
					void createAudience();
				}}
				novalidate
				class="space-y-4"
			>
				{#if errors.audience}<Alert.Root variant="destructive"
						><Alert.Title>บันทึกกลุ่มไม่ได้</Alert.Title><Alert.Description
							>{errors.audience}</Alert.Description
						></Alert.Root
					>{/if}
				<div class="grid gap-4 md:grid-cols-2">
					<div class="space-y-2">
						<Label for="attendance-audience-name"
							>ชื่อกลุ่ม <span aria-hidden="true" class="text-destructive">*</span></Label
						><Input
							id="attendance-audience-name"
							aria-label="ชื่อกลุ่ม"
							required
							bind:value={audienceName}
							aria-invalid={!!fieldErrors.audienceName}
							aria-describedby={fieldErrors.audienceName
								? 'attendance-audience-name-error'
								: undefined}
						/>{#if fieldErrors.audienceName}<p
								id="attendance-audience-name-error"
								class="text-sm text-destructive"
							>
								{fieldErrors.audienceName}
							</p>{/if}
					</div>
					<div class="space-y-2">
						<Label for="attendance-audience-members">เลือกสมาชิก</Label><AttendanceMultiSelect
							id="attendance-audience-members"
							label="สมาชิกกลุ่ม"
							bind:value={audienceStudents}
							options={options.students.map((s) => ({
								value: s.id,
								label: `${s.homeroomName ?? '-'} · ${s.name}`
							}))}
							disabled={busy || archived}
						/>{#if fieldErrors.audienceStudents}<p class="text-sm text-destructive">
								{fieldErrors.audienceStudents}
							</p>{/if}
					</div>
				</div>
				<div class="flex flex-wrap justify-end gap-3">
					{#if audienceId}<Button
							type="button"
							variant="outline"
							onclick={() => {
								audienceId = null;
								audienceVersion = 0;
								audienceName = '';
								audienceStudents = [];
								fieldErrors.audienceName = '';
								fieldErrors.audienceStudents = '';
								errors.audience = '';
							}}>ยกเลิกแก้ไขกลุ่ม</Button
						>{/if}
					<LoadingButton
						type="submit"
						loading={pendingAction === 'audience'}
						disabled={busy || archived}>{audienceId ? 'บันทึกกลุ่ม' : 'เพิ่มกลุ่ม'}</LoadingButton
					>
				</div>
			</form>
			{#if options.audiences.length}<ul class="divide-y rounded-lg border">
					{#each options.audiences as group (group.id)}<li
							class="flex flex-wrap items-center justify-between gap-3 p-3"
						>
							<span class="min-w-0 break-words text-sm"
								>{group.name} · {group.studentIds.length} คน</span
							><Button
								type="button"
								variant="outline"
								size="sm"
								onclick={() => {
									audienceId = group.id;
									audienceVersion = group.rowVersion;
									audienceName = group.name;
									audienceStudents = [...group.studentIds];
									fieldErrors.audienceName = '';
									fieldErrors.audienceStudents = '';
									errors.audience = '';
								}}>แก้ไขกลุ่ม</Button
							>
						</li>{/each}
				</ul>{:else}<p class="text-sm text-muted-foreground">
					ยังไม่มีกลุ่มนักเรียน เพิ่มกลุ่มเพื่อเลือกสมาชิกชุดเดิมในครั้งถัดไป
				</p>{/if}
		</Card.Content>
	</Card.Root>
	<Card.Root>
		<Card.Header
			><Card.Title><h2>เพิ่มรอบเช็คชื่อพิเศษ</h2></Card.Title><Card.Description
				>กำหนดวัน เวลา และผู้รับผิดชอบสำหรับกิจกรรมที่แยกจากรอบปกติ</Card.Description
			></Card.Header
		>
		<Card.Content class="space-y-6">
			<form
				onsubmit={(event) => {
					event.preventDefault();
					void createSpecial();
				}}
				novalidate
				class="space-y-6"
			>
				{#if errors.special}<Alert.Root variant="destructive"
						><Alert.Title>สร้างรอบพิเศษไม่ได้</Alert.Title><Alert.Description
							>{errors.special}</Alert.Description
						></Alert.Root
					>{/if}
				<div class="space-y-2">
					<Label for="attendance-special-title"
						>ชื่อการเช็คชื่อ <span aria-hidden="true" class="text-destructive">*</span></Label
					><Input
						id="attendance-special-title"
						aria-label="ชื่อการเช็คชื่อ"
						bind:value={title}
						required
						placeholder="เช่น ชุมนุม / ทัศนศึกษา"
						aria-invalid={!!fieldErrors.title}
						aria-describedby={fieldErrors.title ? 'attendance-special-title-error' : undefined}
					/>{#if fieldErrors.title}<p
							id="attendance-special-title-error"
							class="text-sm text-destructive"
						>
							{fieldErrors.title}
						</p>{/if}
				</div>
				<div class="grid gap-4 md:grid-cols-4">
					<div class="space-y-2">
						<Label for="attendance-special-start">เริ่มวันที่</Label><DatePicker
							id="attendance-special-start"
							ariaLabel="เริ่มวันที่"
							bind:value={specialStart}
							disabled={busy || archived}
						/>
					</div>
					<div class="space-y-2">
						<Label for="attendance-special-end">ถึงวันที่</Label><DatePicker
							id="attendance-special-end"
							ariaLabel="ถึงวันที่"
							bind:value={specialEnd}
							disabled={busy || archived}
						/>
					</div>
					<div class="space-y-2">
						<Label for="attendance-special-start-time"
							>เวลาเริ่ม <span aria-hidden="true" class="text-destructive">*</span></Label
						><Input
							id="attendance-special-start-time"
							aria-label="เวลาเริ่ม"
							type="time"
							required
							bind:value={startTime}
							aria-invalid={!!fieldErrors.specialTimes}
						/>
					</div>
					<div class="space-y-2">
						<Label for="attendance-special-end-time"
							>เวลาจบ <span aria-hidden="true" class="text-destructive">*</span></Label
						><Input
							id="attendance-special-end-time"
							aria-label="เวลาจบ"
							type="time"
							required
							bind:value={endTime}
							aria-invalid={!!fieldErrors.specialTimes}
							aria-describedby={fieldErrors.specialTimes
								? 'attendance-special-time-error'
								: undefined}
						/>{#if fieldErrors.specialTimes}<p
								id="attendance-special-time-error"
								class="text-sm text-destructive"
							>
								{fieldErrors.specialTimes}
							</p>{/if}
					</div>
				</div>
				<div class="space-y-3">
					<p class="text-sm font-medium">วันในสัปดาห์ที่สร้างรอบ</p>
					<div class="flex flex-wrap gap-3">
						{#each weekdays as day, i (i)}<div class="flex items-center gap-2">
								<Checkbox
									id={`attendance-special-weekday-${i}`}
									checked={repeatDays.includes(i + 1)}
									onCheckedChange={(checked) => {
										repeatDays = checked
											? [...repeatDays, i + 1]
											: repeatDays.filter((day) => day !== i + 1);
									}}
								/><Label for={`attendance-special-weekday-${i}`}>{day}</Label>
							</div>{/each}
					</div>
					{#if fieldErrors.specialDates}<p class="text-sm text-destructive">
							{fieldErrors.specialDates}
						</p>{/if}
				</div>
				<div class="flex flex-wrap gap-4">
					<div class="flex items-center gap-2">
						<Checkbox id="attendance-special-counted" bind:checked={counted} /><Label
							for="attendance-special-counted">นับยอดกิจกรรมนี้</Label
						>
					</div>
					<div class="flex items-center gap-2">
						<Checkbox id="attendance-special-notify" bind:checked={notify} /><Label
							for="attendance-special-notify">แจ้งนักเรียนและผู้ปกครอง</Label
						>
					</div>
				</div>
				<p class="text-sm text-muted-foreground">
					แต่ละวันเป็นหนึ่งรอบ นักเรียนอยู่ได้เพียงกลุ่มเดียวภายในรอบเดียวกัน
					ระบบตรวจกลุ่มซ้ำก่อนบันทึก
				</p>
				{#if fieldErrors.groups}<p class="text-sm text-destructive">{fieldErrors.groups}</p>{/if}
				{#each groups as group, i (i)}
					<section
						aria-label={`กลุ่มในรอบ ${i + 1}`}
						class="space-y-4 rounded-lg border bg-muted/30 p-4"
					>
						<div class="flex flex-wrap items-end gap-3">
							<div class="min-w-0 flex-1 space-y-2">
								<Label for={`attendance-special-group-${i}`}
									>ชื่อกลุ่มในรอบ <span aria-hidden="true" class="text-destructive">*</span></Label
								><Input id={`attendance-special-group-${i}`} bind:value={group.name} required />
							</div>
							<Button
								type="button"
								variant="outline"
								onclick={() => {
									groups = groups.filter((_, index) => index !== i);
								}}
								disabled={groups.length === 1}>เอากลุ่มออก</Button
							>
						</div>
						<div class="flex items-center gap-2">
							<Checkbox
								id={`attendance-special-advisors-${i}`}
								bind:checked={group.useHomeroomAdvisors}
							/><Label for={`attendance-special-advisors-${i}`}
								>ให้ครูประจำชั้นรับผิดชอบห้องที่เลือก</Label
							>
						</div>
						<div class="grid gap-4 md:grid-cols-2">
							<div class="space-y-2">
								<Label for={`attendance-special-rooms-${i}`}>ห้องเรียน</Label><AttendanceMultiSelect
									id={`attendance-special-rooms-${i}`}
									label="ห้องเรียน"
									bind:value={group.homeroomIds}
									options={rooms.map(([value, label]) => ({ value, label }))}
									disabled={busy || archived}
								/>
							</div>
							<div class="space-y-2">
								<Label for={`attendance-special-teachers-${i}`}>ครูที่เลือกเพิ่ม</Label
								><AttendanceMultiSelect
									id={`attendance-special-teachers-${i}`}
									label="ครูที่เลือกเพิ่ม"
									bind:value={group.teacherIds}
									options={options.teachers.map((t) => ({ value: t.id, label: t.name }))}
									disabled={busy || archived}
								/>
							</div>
							<div class="space-y-2">
								<Label for={`attendance-special-audiences-${i}`}>กลุ่มที่ตั้งไว้</Label
								><AttendanceMultiSelect
									id={`attendance-special-audiences-${i}`}
									label="กลุ่มที่ตั้งไว้"
									bind:value={group.audienceGroupIds}
									options={options.audiences.map((a) => ({ value: a.id, label: a.name }))}
									disabled={busy || archived}
								/>
							</div>
							<div class="space-y-2">
								<Label for={`attendance-special-students-${i}`}>นักเรียนรายคน</Label
								><AttendanceMultiSelect
									id={`attendance-special-students-${i}`}
									label="นักเรียนรายคน"
									bind:value={group.studentIds}
									options={options.students.map((s) => ({ value: s.id, label: s.name }))}
									disabled={busy || archived}
								/>
							</div>
						</div>
					</section>
				{/each}
				<div class="flex flex-wrap justify-end gap-3 border-t pt-4">
					<Button
						type="button"
						variant="outline"
						onclick={() => {
							groups = [...groups, { ...newGroup(), name: `กลุ่ม ${groups.length + 1}` }];
						}}>เพิ่มกลุ่มในรอบ</Button
					><LoadingButton
						type="submit"
						loading={pendingAction === 'special'}
						disabled={busy || archived}>สร้างรอบพิเศษ</LoadingButton
					>
				</div>
			</form>
			{#if options.specials.length}<ul class="space-y-2 border-t pt-4">
					{#each options.specials as s (s.id)}<li class="break-words text-sm">
							{s.definition.title} · {s.definition.dates.length} วัน / {s.definition.groups.length} กลุ่ม
						</li>{/each}
				</ul>{/if}
		</Card.Content>
	</Card.Root>
	<Card.Root>
		<Card.Header
			><Card.Title><h2>เครื่องเว็บแคม</h2></Card.Title><Card.Description
				>เครื่องสแกนต้องลงชื่อเข้าใช้บัญชีครูที่มอบหมายและมีสิทธิ์ใช้เครื่องสแกน</Card.Description
			></Card.Header
		>
		<Card.Content class="space-y-6">
			<form
				onsubmit={(event) => {
					event.preventDefault();
					void createDevice();
				}}
				novalidate
				class="space-y-4"
			>
				{#if errors.device}<Alert.Root variant="destructive"
						><Alert.Title>บันทึกเครื่องเว็บแคมไม่ได้</Alert.Title><Alert.Description
							>{errors.device}</Alert.Description
						></Alert.Root
					>{/if}
				<div class="grid gap-4 md:grid-cols-2">
					<div class="space-y-2">
						<Label for="attendance-device-name"
							>ชื่อเครื่อง <span aria-hidden="true" class="text-destructive">*</span></Label
						><Input
							id="attendance-device-name"
							aria-label="ชื่อเครื่อง"
							bind:value={deviceName}
							required
							aria-invalid={!!fieldErrors.deviceName}
							aria-describedby={fieldErrors.deviceName ? 'attendance-device-name-error' : undefined}
						/>{#if fieldErrors.deviceName}<p
								id="attendance-device-name-error"
								class="text-sm text-destructive"
							>
								{fieldErrors.deviceName}
							</p>{/if}
					</div>
					<div class="space-y-2">
						<Label for="attendance-device-operator">ผู้ใช้เครื่อง</Label><AttendanceSelect
							id="attendance-device-operator"
							label="ผู้ใช้เครื่อง"
							placeholder="เลือกผู้ใช้เครื่อง"
							bind:value={operator}
							options={options.teachers.map((t) => ({ value: t.id, label: t.name }))}
							disabled={busy || archived}
						/>{#if fieldErrors.operator}<p class="text-sm text-destructive">
								{fieldErrors.operator}
							</p>{/if}
					</div>
				</div>
				<div class="flex justify-end">
					<LoadingButton
						type="submit"
						loading={pendingAction === 'device'}
						disabled={busy || archived || !operator}>เพิ่มเครื่อง</LoadingButton
					>
				</div>
			</form>
			{#if options.devices.length}<ul class="divide-y rounded-lg border">
					{#each options.devices as d (d.id)}<li
							class="flex flex-wrap items-center justify-between gap-3 p-3"
						>
							<div class="min-w-0 space-y-1">
								<p class="break-words font-medium">{d.name}</p>
								<p class="break-words text-sm text-muted-foreground">
									{options.teachers.find((t) => t.id === d.operatorId)?.name ?? 'ผู้ใช้ที่มอบหมาย'}
								</p>
							</div>
							<div class="flex items-center gap-3">
								<Badge variant={d.enabled ? 'secondary' : 'outline'}
									>{d.enabled ? 'เปิดใช้งาน' : 'ปิดใช้งาน'}</Badge
								><LoadingButton
									type="button"
									variant="outline"
									loading={pendingAction === `device-toggle:${d.id}`}
									disabled={busy || archived}
									onclick={() => toggleDevice(d.id)}
									>{d.enabled ? 'ปิดเครื่อง' : 'เปิดเครื่อง'}</LoadingButton
								>
							</div>
						</li>{/each}
				</ul>{:else}<p class="text-sm text-muted-foreground">
					ยังไม่มีเครื่องเว็บแคม เพิ่มเครื่องและเลือกบัญชีครูผู้ใช้เครื่องก่อนเริ่มสแกน
				</p>{/if}
		</Card.Content>
	</Card.Root>
</fieldset>
