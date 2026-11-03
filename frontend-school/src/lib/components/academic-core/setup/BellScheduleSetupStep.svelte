<script lang="ts">
	import type {
		AcademicYear,
		BellSchedule,
		CreateBellScheduleRequest,
		CloneBellScheduleRequest,
		UpdateBellScheduleRequest
	} from '#lib/api/academic-core.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Check, Clock3, Pencil, Plus, Save, X } from '@lucide/svelte';
	import { bellScheduleBlockMessage } from '#lib/academic-core/period-setup.js';

	let {
		year,
		schedules,
		busy = false,
		canManage = false,
		onCreate,
		onClone,
		onUpdate,
		onCloned,
		onSaved
	}: {
		year: AcademicYear;
		schedules: BellSchedule[];
		busy?: boolean;
		canManage?: boolean;
		onCreate: (draft: CreateBellScheduleRequest) => Promise<BellSchedule>;
		onClone: (id: string, draft: CloneBellScheduleRequest) => Promise<BellSchedule>;
		onUpdate: (id: string, draft: UpdateBellScheduleRequest) => Promise<BellSchedule>;
		onCloned: (schedule: BellSchedule) => void;
		onSaved: (schedule: BellSchedule) => void;
	} = $props();

	let name = $derived(schedules.length === 0 ? 'ตารางเวลาปกติ' : '');
	let editing = $state<BellSchedule | null>(null);
	let editName = $state('');
	let errorMessage = $state('');
	let cloning = $state<BellSchedule | null>(null);
	let cloneName = $state('');
	const defaultProtected = $derived(
		schedules.some((schedule) => schedule.isDefault && schedule.editBlockedReason)
	);
	const latestEditing = $derived(schedules.find((schedule) => schedule.id === editing?.id));
	const editChanged = $derived(!!editing && latestEditing?.rowVersion !== editing.rowVersion);

	async function copySchedule(event: SubmitEvent) {
		event.preventDefault();
		if (!cloning || !cloneName.trim() || !canManage || busy) return;
		errorMessage = '';
		try {
			const saved = await onClone(cloning.id, {
				academicYearId: year.id,
				name: cloneName.trim(),
				owningOrganizationUnitId: cloning.owningOrganizationUnitId ?? null,
				rowVersion: cloning.rowVersion
			});
			cloning = null;
			onCloned(saved);
		} catch (cause) {
			errorMessage = cause instanceof Error ? cause.message : 'คัดลอกตารางเวลาไม่สำเร็จ';
		}
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (!canManage || busy) return;
		errorMessage = '';
		if (!name.trim()) {
			errorMessage = 'กรุณาระบุชื่อตารางเวลาที่ครูเข้าใจได้';
			return;
		}
		try {
			const saved = await onCreate({
				academicYearId: year.id,
				name: name.trim(),
				owningOrganizationUnitId: null
			});
			name = '';
			onSaved(saved);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างตารางเวลาไม่สำเร็จ';
		}
	}

	function beginEdit(schedule: BellSchedule) {
		if (!canManage || schedule.editBlockedReason || busy) return;
		editing = { ...schedule };
		editName = schedule.name;
		errorMessage = '';
	}

	async function saveEdit(event: SubmitEvent) {
		event.preventDefault();
		if (
			!editing ||
			!editName.trim() ||
			!canManage ||
			editChanged ||
			latestEditing?.editBlockedReason ||
			busy
		)
			return;
		try {
			const saved = await onUpdate(editing.id, {
				name: editName.trim(),
				isDefault: editing.isDefault,
				owningOrganizationUnitId: editing.owningOrganizationUnitId ?? null,
				rowVersion: editing.rowVersion
			});
			editing = null;
			onSaved(saved);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'แก้ไขตารางเวลาไม่สำเร็จ';
		}
	}

	async function makeDefault(schedule: BellSchedule) {
		if (!canManage || schedule.editBlockedReason || defaultProtected || busy) return;
		errorMessage = '';
		try {
			const saved = await onUpdate(schedule.id, {
				name: schedule.name,
				isDefault: true,
				owningOrganizationUnitId: schedule.owningOrganizationUnitId ?? null,
				rowVersion: schedule.rowVersion
			});
			onSaved(saved);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'ตั้งตารางหลักไม่สำเร็จ';
		}
	}
</script>

<div class="space-y-5">
	<div class="rounded-xl border bg-muted/20 p-4">
		<p class="text-xs font-medium text-muted-foreground">ปีที่กำลังตั้งค่า</p>
		<p class="mt-1 font-semibold">{year.name}</p>
		<p class="mt-1 text-xs text-muted-foreground">
			ตารางหลักเป็นเพียงตัวเลือกเริ่มต้นเมื่อสร้างภาคเรียน ไม่ได้เปิดใช้งานปีการศึกษา
		</p>
	</div>

	<div class="space-y-2">
		{#each schedules as schedule (schedule.id)}
			<div
				class="flex flex-wrap items-center justify-between gap-3 rounded-xl border bg-background p-4"
			>
				<div class="flex min-w-0 items-center gap-3">
					<div class="rounded-lg bg-primary/10 p-2 text-primary">
						<Clock3 class="size-4" aria-hidden="true" />
					</div>
					<div class="min-w-0">
						<p class="truncate font-medium">{schedule.name}</p>
						<p class="text-xs text-muted-foreground">
							{bellScheduleBlockMessage(schedule.editBlockedReason) ||
								'ตั้งค่าคาบเรียนได้ก่อนเริ่มใช้ภาคเรียน'}
						</p>
					</div>
				</div>
				<div class="flex items-center gap-2">
					{#if schedule.isDefault}
						<Badge><Check class="size-3" /> ตารางหลัก</Badge>
					{:else if canManage && !schedule.editBlockedReason && !defaultProtected}
						<Button
							type="button"
							size="sm"
							variant="outline"
							disabled={busy}
							onclick={() => void makeDefault(schedule)}
						>
							ตั้งเป็นตารางหลัก
						</Button>
					{/if}
					{#if canManage && !schedule.editBlockedReason}<Button
							type="button"
							size="icon-sm"
							variant="ghost"
							onclick={() => beginEdit(schedule)}
							aria-label={`แก้ไข ${schedule.name}`}
						>
							<Pencil class="size-4" />
						</Button>{/if}
					{#if canManage && schedule.canClone}<Button
							type="button"
							variant="outline"
							size="sm"
							disabled={busy}
							onclick={() => {
								cloning = { ...schedule };
								cloneName = `${schedule.name} สำเนา`;
								errorMessage = '';
							}}>คัดลอก {schedule.name}</Button
						>{/if}
				</div>
			</div>
		{:else}
			<p class="rounded-xl border border-dashed p-5 text-sm text-muted-foreground">
				ยังไม่มีตารางเวลา เริ่มจากตารางปกติของโรงเรียนก่อน แล้วค่อยเพิ่มตารางพิเศษภายหลัง
			</p>
		{/each}
	</div>

	{#if cloning}<form class="space-y-3 rounded-xl border p-4" onsubmit={copySchedule}>
			<Label for="clone-schedule-name">ชื่อตารางที่คัดลอก</Label><Input
				id="clone-schedule-name"
				bind:value={cloneName}
				required
				disabled={busy}
			/>
			<p class="text-sm text-muted-foreground">
				คัดลอกคาบไปยังตารางใหม่โดยเก็บตารางเดิมไว้ แล้วเลือกตารางใหม่ให้ภาคเรียนที่ยังไม่เริ่มใช้
				หากมีฉบับร่างตารางสอนเดิม ต้องจัดการฉบับร่างก่อนเปลี่ยนตารางของภาคเรียน
			</p>
			<div class="flex gap-2">
				<Button type="submit" disabled={busy}>ยืนยันคัดลอกตาราง</Button><Button
					type="button"
					variant="outline"
					disabled={busy}
					onclick={() => {
						cloning = null;
					}}>ยกเลิก</Button
				>
			</div>
		</form>{/if}
	{#if editing && canManage}
		<form class="space-y-3 rounded-xl border border-primary/30 p-4" onsubmit={saveEdit}>
			<div class="flex items-center justify-between gap-3">
				<Label for="bell-schedule-edit-name">แก้ไขชื่อตารางเวลา</Label>
				<Button
					type="button"
					size="icon-sm"
					variant="ghost"
					onclick={() => (editing = null)}
					aria-label="ยกเลิกการแก้ไข"
				>
					<X class="size-4" />
				</Button>
			</div>
			<Input id="bell-schedule-edit-name" bind:value={editName} required />
			{#if editChanged}<p role="alert" class="text-sm text-destructive">
					ตารางเวลาเปลี่ยนระหว่างแก้ไข ฉบับร่างของคุณยังอยู่ ตรวจข้อมูลล่าสุดก่อนเริ่มแก้ไขอีกครั้ง
				</p>{/if}
			{#if !latestEditing?.editBlockedReason}<Button
					type="submit"
					size="sm"
					disabled={busy || editChanged}><Save class="size-4" /> บันทึกชื่อ</Button
				>{/if}
		</form>
	{/if}

	{#if canManage}<form class="space-y-3 rounded-xl border bg-card p-4" onsubmit={submit}>
			<div>
				<Label for="bell-schedule-name">เพิ่มตารางเวลา</Label>
				<p class="mt-1 text-xs text-muted-foreground">
					ตั้งชื่อตามการใช้งาน เช่น ตารางเวลาปกติ หรือตารางวันศุกร์
				</p>
			</div>
			<div class="flex flex-col gap-2 sm:flex-row">
				<Input
					id="bell-schedule-name"
					class="flex-1"
					bind:value={name}
					placeholder="ตารางเวลาปกติ"
					required
				/>
				<Button type="submit" disabled={busy}><Plus class="size-4" /> เพิ่มตารางเวลา</Button>
			</div>
		</form>{/if}

	{#if errorMessage}
		<p
			role="alert"
			class="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
		>
			{errorMessage}
		</p>
	{/if}
</div>
