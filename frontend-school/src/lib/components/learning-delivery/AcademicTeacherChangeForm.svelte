<script lang="ts">
	import {
		upsertAcademicTermChangeItem,
		type AcademicTermChangeSet,
		type DeliveryManagementOptions,
		type LearningTeacherRole,
		type UpsertAcademicTermChangeItemRequest
	} from '$lib/api/learning-delivery';
	import { ApiClientError } from '$lib/api/client';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { ArrowRight, UserRoundPlus, X } from 'lucide-svelte';
	import DeliveryOptionCombobox from './DeliveryOptionCombobox.svelte';

	type TeacherAction = 'add_group_teacher' | 'adjust_group_teacher_role' | 'stop_group_teacher';

	let {
		changeSet,
		managementOptions,
		onSaved,
		onConflict,
		onCancel
	}: {
		changeSet: AcademicTermChangeSet;
		managementOptions: DeliveryManagementOptions;
		onSaved: (changeSet: AcademicTermChangeSet) => void | Promise<void>;
		onConflict: (message: string) => void | Promise<void>;
		onCancel: () => void;
	} = $props();

	let action = $state<TeacherAction>('add_group_teacher');
	let learningGroupId = $state('');
	let learningGroupTeacherId = $state('');
	let teacherId = $state('');
	let teacherRole = $state<LearningTeacherRole>('primary');
	let saving = $state(false);
	let errorMessage = $state('');

	let publishedGroups = $derived(
		managementOptions.learningGroups.filter((group) => group.status === 'published')
	);
	let groupOptions = $derived(
		publishedGroups.map((group) => ({
			id: group.id,
			label: `${group.code} — ${group.name}`,
			description: `${group.teacherAssignments.length} ช่วงความรับผิดชอบ`
		}))
	);
	let selectedGroup = $derived(
		publishedGroups.find((group) => group.id === learningGroupId) ?? null
	);
	let pendingEpisodeIds = $derived.by(() => {
		return changeSet.items.flatMap((item) =>
			(item.actionKind === 'adjust_group_teacher_role' ||
				item.actionKind === 'stop_group_teacher') &&
			item.learningGroupTeacherId
				? [item.learningGroupTeacherId]
				: []
		);
	});
	let effectiveAssignments = $derived(
		(selectedGroup?.teacherAssignments ?? []).filter(
			(assignment) =>
				assignment.startsOn < changeSet.effectiveFrom &&
				(!assignment.endsOn || assignment.endsOn >= changeSet.effectiveFrom) &&
				!pendingEpisodeIds.includes(assignment.id)
		)
	);
	let assignmentOptions = $derived(
		effectiveAssignments.map((assignment) => ({
			id: assignment.id,
			label: assignment.displayName,
			description: `${roleLabel(assignment.role)} · เริ่ม ${formatDate(assignment.startsOn)}`
		}))
	);
	let selectedAssignment = $derived(
		effectiveAssignments.find((assignment) => assignment.id === learningGroupTeacherId) ?? null
	);
	let activeTeacherIds = $derived(
		(selectedGroup?.teacherAssignments ?? [])
			.filter(
				(assignment) =>
					assignment.startsOn <= changeSet.effectiveFrom &&
					(!assignment.endsOn || assignment.endsOn >= changeSet.effectiveFrom)
			)
			.map((assignment) => assignment.teacherId)
	);
	let pendingAddedTeacherIds = $derived(
		changeSet.items.flatMap((item) =>
			item.actionKind === 'add_group_teacher' && item.learningGroupId === learningGroupId
				? [item.teacherId]
				: []
		)
	);
	let teacherOptions = $derived(
		managementOptions.teachers
			.filter(
				(teacher) =>
					!activeTeacherIds.includes(teacher.id) && !pendingAddedTeacherIds.includes(teacher.id)
			)
			.map((teacher) => ({
				id: teacher.id,
				label: teacher.name,
				description: teacher.title
			}))
	);
	let request = $derived.by(buildRequest);

	function roleLabel(role: LearningTeacherRole): string {
		return role === 'primary' ? 'ครูหลัก' : role === 'secondary' ? 'ครูร่วม' : 'ครูผู้ช่วย';
	}

	function actionLabel(value: TeacherAction): string {
		return value === 'add_group_teacher'
			? 'เพิ่มครูในกลุ่มเรียน'
			: value === 'adjust_group_teacher_role'
				? 'ปรับบทบาทความรับผิดชอบ'
				: 'หยุดความรับผิดชอบของครู';
	}

	function formatDate(value: string): string {
		return new Intl.DateTimeFormat('th-TH', { dateStyle: 'medium' }).format(
			new Date(`${value}T00:00:00`)
		);
	}

	function resetSelection(nextAction: TeacherAction = action) {
		action = nextAction;
		learningGroupId = '';
		learningGroupTeacherId = '';
		teacherId = '';
		teacherRole = 'primary';
		errorMessage = '';
	}

	function changeGroup(value: string) {
		learningGroupId = value;
		learningGroupTeacherId = '';
		teacherId = '';
		teacherRole = 'primary';
	}

	function changeAssignment(value: string) {
		learningGroupTeacherId = value;
		const assignment = effectiveAssignments.find((item) => item.id === value);
		teacherId = assignment?.teacherId ?? '';
		teacherRole = assignment?.role ?? 'primary';
	}

	function buildRequest(): UpsertAcademicTermChangeItemRequest | null {
		if (!learningGroupId) return null;
		if (action === 'add_group_teacher') {
			if (!teacherId) return null;
			return {
				action,
				changeSetRowVersion: changeSet.rowVersion,
				itemRowVersion: null,
				learningGroupId,
				teacherId,
				teacherRole
			};
		}
		if (!selectedAssignment || !teacherId) return null;
		if (action === 'adjust_group_teacher_role') {
			if (teacherRole === selectedAssignment.role) return null;
			return {
				action,
				changeSetRowVersion: changeSet.rowVersion,
				itemRowVersion: null,
				learningGroupId,
				learningGroupTeacherId,
				teacherId,
				teacherRole
			};
		}
		return {
			action,
			changeSetRowVersion: changeSet.rowVersion,
			itemRowVersion: null,
			learningGroupId,
			learningGroupTeacherId,
			teacherId
		};
	}

	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (!request) return;
		saving = true;
		errorMessage = '';
		try {
			const updated = await upsertAcademicTermChangeItem(changeSet.id, request);
			await onSaved(updated);
		} catch (error) {
			if (error instanceof ApiClientError && error.status === 409) {
				await onConflict('ข้อมูลกลุ่มเรียนหรือชุดการเปลี่ยนแปลงเปลี่ยนไป กรุณาตรวจรายการล่าสุด');
				return;
			}
			errorMessage = error instanceof Error ? error.message : 'บันทึกการเปลี่ยนครูไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}
</script>

<form class="space-y-4 rounded-xl border border-sky-500/25 bg-sky-500/[0.035] p-4" onsubmit={save}>
	<div class="flex items-start justify-between gap-3">
		<div class="flex min-w-0 items-start gap-3">
			<div class="rounded-lg bg-sky-500/12 p-2 text-sky-700">
				<UserRoundPlus class="size-4" />
			</div>
			<div>
				<h3 class="font-medium">เปลี่ยนครูผู้สอนกลางภาค</h3>
				<p class="text-xs leading-5 text-muted-foreground">
					บันทึกเป็นแบบร่างก่อน และเริ่มมีผลวันที่ {formatDate(changeSet.effectiveFrom)}
					หลังเผยแพร่เท่านั้น
				</p>
			</div>
		</div>
		<Button type="button" size="icon" variant="ghost" onclick={onCancel} aria-label="ปิดแบบฟอร์ม">
			<X class="size-4" />
		</Button>
	</div>

	<div class="grid gap-4 lg:grid-cols-2">
		<div class="space-y-2">
			<Label>สิ่งที่ต้องการเปลี่ยน</Label>
			<Select.Root
				type="single"
				value={action}
				onValueChange={(value) => resetSelection(value as TeacherAction)}
			>
				<Select.Trigger class="w-full">{actionLabel(action)}</Select.Trigger>
				<Select.Content>
					<Select.Item value="add_group_teacher">เพิ่มครูในกลุ่มเรียน</Select.Item>
					<Select.Item value="adjust_group_teacher_role">ปรับบทบาทความรับผิดชอบ</Select.Item>
					<Select.Item value="stop_group_teacher">หยุดความรับผิดชอบของครู</Select.Item>
				</Select.Content>
			</Select.Root>
		</div>
		<div class="space-y-2">
			<Label>กลุ่มเรียน</Label>
			<DeliveryOptionCombobox
				bind:value={() => learningGroupId, changeGroup}
				options={groupOptions}
				placeholder="ค้นหารหัสหรือชื่อกลุ่มเรียน"
				searchPlaceholder="ค้นหากลุ่มเรียน..."
			/>
		</div>
	</div>

	{#if learningGroupId}
		<div class="grid gap-4 lg:grid-cols-2">
			{#if action === 'add_group_teacher'}
				<div class="space-y-2">
					<Label>ครูที่จะเพิ่ม</Label>
					<DeliveryOptionCombobox
						bind:value={teacherId}
						options={teacherOptions}
						placeholder="ค้นหาชื่อครู"
						searchPlaceholder="ค้นหาครู..."
					/>
				</div>
			{:else}
				<div class="space-y-2">
					<Label>ครูและช่วงความรับผิดชอบเดิม</Label>
					<DeliveryOptionCombobox
						bind:value={() => learningGroupTeacherId, changeAssignment}
						options={assignmentOptions}
						placeholder="เลือกครูจากช่วงที่ใช้งานอยู่"
						searchPlaceholder="ค้นหาครู..."
					/>
				</div>
			{/if}
			{#if action !== 'stop_group_teacher'}
				<div class="space-y-2">
					<Label>บทบาทหลังเริ่มใช้</Label>
					<Select.Root
						type="single"
						value={teacherRole}
						onValueChange={(value) => (teacherRole = value as LearningTeacherRole)}
					>
						<Select.Trigger class="w-full">{roleLabel(teacherRole)}</Select.Trigger>
						<Select.Content>
							<Select.Item value="primary">ครูหลัก</Select.Item>
							<Select.Item value="secondary">ครูร่วม</Select.Item>
							<Select.Item value="assistant">ครูผู้ช่วย</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>
			{/if}
		</div>
	{/if}

	{#if selectedGroup && (selectedAssignment || (action === 'add_group_teacher' && teacherId))}
		<div
			class="grid gap-2 rounded-xl border bg-background p-3 sm:grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] sm:items-center"
		>
			<div class="min-w-0">
				<p class="text-xs text-muted-foreground">ก่อนวันที่เริ่มใช้</p>
				<p class="truncate font-medium">
					{selectedAssignment
						? `${selectedAssignment.displayName} · ${roleLabel(selectedAssignment.role)}`
						: 'ข้อมูลครูเดิมยังคงอยู่'}
				</p>
			</div>
			<ArrowRight class="hidden size-4 text-sky-700 sm:block" />
			<div class="min-w-0">
				<p class="text-xs text-muted-foreground">หลังเผยแพร่และถึงวันที่เริ่มใช้</p>
				<p class="truncate font-medium text-sky-800">
					{action === 'stop_group_teacher'
						? 'สิ้นสุดความรับผิดชอบ (คาบเรียนต้องส่งต่อแยกต่างหาก)'
						: `${managementOptions.teachers.find((teacher) => teacher.id === teacherId)?.name ?? selectedAssignment?.displayName ?? 'ครูที่เลือก'} · ${roleLabel(teacherRole)}`}
				</p>
			</div>
		</div>
	{/if}

	{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
	<div class="flex justify-end gap-2">
		<Button type="button" variant="outline" onclick={onCancel}>ยกเลิก</Button>
		<LoadingButton type="submit" loading={saving} loadingLabel="กำลังบันทึก" disabled={!request}>
			บันทึกการเปลี่ยนครู
		</LoadingButton>
	</div>
</form>
