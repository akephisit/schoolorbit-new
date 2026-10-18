<script lang="ts">
	import { CareMutationIdentity } from '#lib/features/student-care/mutation.js';
	const mutation = new CareMutationIdentity();

	import { onDestroy } from 'svelte';
	import {
		createCareAssignments,
		type CareRound,
		type CareInstrumentVersion,
		type CareStudentOption,
		type CareStaffOption,
		type CareAssignment
	} from '#lib/api/student-care.js';
	import { careIdentity, careManagePermissions } from '#lib/features/student-care/access.js';
	import { respondentLabels } from '#lib/features/student-care/presentation.js';
	import { careLink } from '#lib/features/student-care/presentation.js';
	import { ClipboardList, GraduationCap, UserRoundCheck, Send } from '@lucide/svelte';
	import { Button } from '#lib/components/ui/button/index.js';

	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';

	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import CareSection from './CareSection.svelte';
	import CareSelect from './CareSelect.svelte';
	import CareStaffPicker from './CareStaffPicker.svelte';
	import CareStudentPicker from './CareStudentPicker.svelte';
	let {
		round,
		instruments,
		students,
		staff,
		oncreated
	}: {
		round: CareRound;
		instruments: CareInstrumentVersion[];
		students: CareStudentOption[];
		staff: CareStaffOption[];
		oncreated?: (assignments: CareAssignment[]) => void;
	} = $props();
	let versionId = $state(''),
		selected = $state<CareStudentOption[]>([]),
		reviewers = $state<string[]>([]),
		teachers = $state<string[]>([]),
		pending = $state(false),
		error = $state(''),
		message = $state('');
	const instrument = $derived(instruments.find((i) => i.id === versionId));
	let alive = true;
	onDestroy(() => (alive = false));
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending || !$can.hasAny(...careManagePermissions)) return;
		if (!instrument || !selected.length || !reviewers.length) {
			error = 'เลือกรุ่นแบบประเมิน นักเรียน และผู้รับผิดชอบติดตาม';
			return;
		}
		const targets = selected.flatMap((student) =>
			instrument.definition.respondent === 'self_report'
				? [{ studentId: student.id, respondentId: student.id }]
				: instrument.definition.respondent === 'guardian'
					? student.guardianIds.map((respondentId) => ({ studentId: student.id, respondentId }))
					: teachers
							.filter(
								(id) =>
									$can.has(PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL) ||
									student.advisorIds.includes(id)
							)
							.map((respondentId) => ({ studentId: student.id, respondentId }))
		);
		if (
			!targets.length ||
			(instrument.definition.respondent === 'teacher' &&
				!$can.has(PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL) &&
				selected.some((student) => !teachers.some((id) => student.advisorIds.includes(id)))) ||
			selected.some(
				(student) => instrument.definition.respondent === 'guardian' && !student.guardianIds.length
			)
		) {
			error =
				instrument.definition.respondent === 'guardian'
					? 'มีนักเรียนที่ยังไม่มีผู้ปกครองเชื่อมโยง กรุณาตรวจสอบรายชื่อ'
					: 'เลือกครูผู้ตอบอย่างน้อยหนึ่งคน';
			return;
		}
		if (targets.length > 100 || reviewers.length > 30) {
			error = 'มอบหมายได้ครั้งละไม่เกิน 100 ผู้ตอบ และผู้รับผิดชอบไม่เกิน 30 คน';
			return;
		}
		pending = true;
		error = '';
		message = '';
		const identity = $careIdentity;
		try {
			const result = await createCareAssignments(
				mutation.prepare(
					{
						requestId: crypto.randomUUID(),
						roundId: round.id,
						instrumentVersionId: instrument.id,
						targets,
						reviewerUserIds: reviewers
					},
					'createCareAssignments'
				)
			);
			if (alive && identity === $careIdentity) {
				message = `มอบหมายแล้ว ${result.length} รายการ`;
				selected = [];
				oncreated?.(result);
			}
		} catch (cause) {
			if (alive && identity === $careIdentity)
				error = cause instanceof Error ? cause.message : 'มอบหมายไม่ได้';
		} finally {
			if (alive && identity === $careIdentity) pending = false;
		}
	}
</script>

<form onsubmit={save} class="max-w-4xl space-y-5">
	<CareSection title="1. เลือกแบบประเมิน" icon={ClipboardList} description={round.name}>
		<CareSelect
			id="assignment-instrument"
			label="รุ่นแบบประเมิน"
			value={versionId}
			options={instruments
				.filter((i) => round.instrumentVersionIds.includes(i.id))
				.map((i) => ({
					value: i.id,
					label: `${i.definition.name} · ${respondentLabels[i.definition.respondent]} · รุ่น ${i.version}`
				}))}
			onValueChange={(value) => (versionId = value)}
		/>
	</CareSection><CareSection
		title="2. เลือกนักเรียนและผู้ตอบ"
		icon={GraduationCap}
		description="เลือกนักเรียนจากรายชื่อ ณ วันที่อ้างอิงของรอบนี้"
	>
		<CareStudentPicker
			initial={students}
			term={round.academicTermId}
			date={round.rosterDate}
			value={selected}
			onchange={(value) => (selected = value)}
			multiple
		/>
		{#if instrument?.definition.respondent === 'guardian'}<p class="text-sm text-muted-foreground">
				มอบหมายให้ผู้ปกครองที่เชื่อมโยงอยู่ทุกคนของนักเรียนที่เลือก
			</p>{:else if instrument?.definition.respondent === 'teacher'}<CareStaffPicker
				staff={staff.filter(
					(person) =>
						$can.has(PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL) ||
						selected.some((student) => student.advisorIds.includes(person.id))
				)}
				value={teachers}
				onchange={(value) => (teachers = value)}
				label="ครูผู้ตอบ"
			/>{/if}
	</CareSection><CareSection
		title="3. ระบุผู้รับผิดชอบติดตามผล"
		icon={UserRoundCheck}
		description="ระบุชื่อบุคลากรที่จะติดตามผลโดยตรง การเป็นครูประจำชั้นไม่ได้ทำให้เข้าถึงคำตอบโดยอัตโนมัติ"
	>
		<CareStaffPicker
			{staff}
			value={reviewers}
			onchange={(value) => (reviewers = value)}
			label="ผู้รับผิดชอบติดตามผล"
		/>
	</CareSection>{#if error}<PageState
			variant="error"
			title="มอบหมายไม่ได้"
			description={error}
		/>{/if}{#if message}<p role="status" class="text-sm text-success">{message}</p>{/if}
	<div class="flex flex-wrap items-center justify-between gap-3">
		<p class="text-sm text-muted-foreground">
			เลือกนักเรียน {selected.length} คน · ผู้รับผิดชอบ {reviewers.length} คน
		</p>
		<div class="flex flex-wrap justify-end gap-2">
			<Button variant="outline" href={careLink(`/rounds/${round.id}`, round.academicTermId)}
				>ยกเลิก</Button
			>
			<LoadingButton type="submit" loading={pending} disabled={round.status !== 'open'}
				><Send aria-hidden="true" />มอบหมายแบบประเมิน</LoadingButton
			>
		</div>
	</div>
</form>
