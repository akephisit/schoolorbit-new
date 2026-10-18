<script lang="ts">
	import { HeartHandshake, UserRoundCheck, NotebookPen, Save } from '@lucide/svelte';
	import { CareMutationIdentity } from '#lib/features/student-care/mutation.js';
	const mutation = new CareMutationIdentity();

	import { onDestroy, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		saveCareCase,
		type CareStudentOption,
		type CareStaffOption,
		type CareCaseDetail
	} from '#lib/api/student-care.js';
	import {
		mayManageCareCase,
		careIdentity,
		careSensitiveManagePermissions
	} from '#lib/features/student-care/access.js';
	import { careLink } from '#lib/features/student-care/presentation.js';
	import { authStore } from '#lib/stores/auth.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import CareSection from './CareSection.svelte';
	import CareStaffPicker from './CareStaffPicker.svelte';
	import CareStudentPicker from './CareStudentPicker.svelte';
	let {
		initial,
		students,
		staff,
		term,
		date,
		sourceAssignmentId = null,
		sourceVisitId = null,
		studentId = null
	}: {
		initial?: CareCaseDetail;
		students: CareStudentOption[];
		staff: CareStaffOption[];
		term: string;
		date: string;
		sourceAssignmentId?: string | null;
		sourceVisitId?: string | null;
		studentId?: string | null;
	} = $props();
	let selected = $state<CareStudentOption[]>(
			untrack(() => students.filter((s) => s.id === studentId))
		),
		title = $state(untrack(() => initial?.case.title ?? '')),
		assignees = $state<string[]>(untrack(() => initial?.case.assignedUserIds ?? [])),
		nextFollowUp = $state(untrack(() => initial?.case.nextFollowUp ?? '')),
		note = $state(''),
		pending = $state(false),
		error = $state('');
	let alive = true;
	onDestroy(() => (alive = false));
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (
			pending ||
			!$can.hasAny(...careSensitiveManagePermissions) ||
			(initial && (!mayManageCareCase(initial.case) || initial.case.status === 'closed'))
		)
			return;
		const selectedId = initial?.case.studentId ?? studentId ?? selected[0]?.id;
		if (!title.trim() || !note.trim()) {
			error = 'กรอกเรื่องที่ดูแลและรายละเอียดให้ครบ';
			return;
		}
		if (!selectedId || !assignees.length) {
			error = 'เลือกนักเรียนและระบุผู้รับผิดชอบอย่างน้อยหนึ่งคน';
			return;
		}
		if (
			!$can.has(PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL) &&
			$authStore.user &&
			!assignees.includes($authStore.user.id)
		) {
			error = 'ผู้เปิดการดูแลต้องอยู่ในรายชื่อผู้รับผิดชอบด้วย';
			return;
		}
		pending = true;
		error = '';
		const identity = $careIdentity;
		try {
			const result = await saveCareCase(
				mutation.prepare(
					{
						requestId: crypto.randomUUID(),
						rowVersion: initial?.case.rowVersion ?? null,
						studentId: selectedId,
						title: title.trim(),
						assignedUserIds: assignees,
						nextFollowUp: nextFollowUp || null,
						note: note.trim(),
						sourceAssignmentId: initial?.case.sourceAssignmentId ?? sourceAssignmentId,
						sourceVisitId: initial?.case.sourceVisitId ?? sourceVisitId
					},
					'saveCareCase'
				),
				initial?.case.id
			);
			if (alive && identity === $careIdentity)
				await goto(careLink(`/cases/${result.case.id}`, term));
		} catch (cause) {
			if (alive && identity === $careIdentity)
				error = cause instanceof Error ? cause.message : 'บันทึกไม่ได้';
		} finally {
			if (alive && identity === $careIdentity) pending = false;
		}
	}
</script>

<form onsubmit={save} class="max-w-4xl space-y-5">
	<CareSection
		title="1. เรื่องที่ต้องดูแล"
		icon={HeartHandshake}
		description={initial?.case.studentName ?? 'ระบุนักเรียนและเรื่องที่ต้องการให้ความช่วยเหลือ'}
	>
		{#if !initial && !studentId}<CareStudentPicker
				initial={students}
				{term}
				{date}
				value={selected}
				onchange={(value) => (selected = value)}
			/>{/if}
		{#if sourceAssignmentId || sourceVisitId}<p class="rounded-lg bg-primary/5 p-3 text-sm">
				เชื่อมโยงการดูแลกับ{sourceAssignmentId ? 'แบบประเมิน' : 'การเยี่ยมบ้าน'}ต้นทาง
			</p>{/if}
		<div class="space-y-2">
			<Label for="case-title">เรื่องที่ดูแล *</Label><Input
				id="case-title"
				bind:value={title}
				required
				maxlength={200}
			/>
		</div>
	</CareSection><CareSection
		title="2. ผู้รับผิดชอบและการติดตาม"
		icon={UserRoundCheck}
		description="ระบุผู้รับผิดชอบโดยตรง พร้อมกำหนดติดตามเมื่อทราบวันนัดหมาย"
	>
		<CareStaffPicker
			{staff}
			value={assignees}
			onchange={(value) => (assignees = value)}
			label="ผู้รับผิดชอบการดูแล"
			disabled={!!initial && !$can.has(PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL)}
		/>
		<div class="space-y-2">
			<Label for="case-followup">กำหนดติดตาม</Label><DatePicker
				id="case-followup"
				ariaLabel="กำหนดติดตาม"
				value={nextFollowUp}
				onValueChange={(value) => (nextFollowUp = value ?? '')}
				clearable
			/>
		</div>
	</CareSection><CareSection
		title={initial ? '3. เหตุผลการเปลี่ยนแผน' : '3. บันทึกเริ่มต้น'}
		icon={NotebookPen}
		description="รายละเอียดนี้จะเก็บเป็นประวัติการดูแล ไม่ได้แทนที่บันทึกเดิม"
	>
		<div class="space-y-2">
			<Label for="case-note">รายละเอียด / เหตุผล *</Label><Textarea
				id="case-note"
				bind:value={note}
				required
				maxlength={5000}
			/>
		</div>
	</CareSection>{#if error}<PageState
			variant="error"
			title="บันทึกการดูแลไม่ได้"
			description={error}
		/>{/if}
	<div class="flex justify-end gap-2">
		<Button
			variant="outline"
			href={careLink(initial ? `/cases/${initial.case.id}` : '/cases', term)}>ยกเลิก</Button
		><LoadingButton
			type="submit"
			loading={pending}
			disabled={!!initial && (!mayManageCareCase(initial.case) || initial.case.status === 'closed')}
			><Save aria-hidden="true" />{initial
				? 'บันทึกผู้รับผิดชอบและแผน'
				: 'เปิดการดูแล'}</LoadingButton
		>
	</div>
</form>
