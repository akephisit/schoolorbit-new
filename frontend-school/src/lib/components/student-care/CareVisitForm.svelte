<script lang="ts">
	import { House, NotebookPen, Save } from '@lucide/svelte';
	import { CareMutationIdentity } from '#lib/features/student-care/mutation.js';
	const mutation = new CareMutationIdentity();

	import { untrack, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		careStudents,
		saveCareVisit,
		type CareVisitRecord,
		type CareStudentOption,
		type CareStaffOption,
		type CareVisitStatus
	} from '#lib/api/student-care.js';
	import {
		mayManageCareVisit,
		careIdentity,
		careManagePermissions
	} from '#lib/features/student-care/access.js';
	import { careLink, visitLabels } from '#lib/features/student-care/presentation.js';
	import { authStore } from '#lib/stores/auth.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { PageSkeleton } from '#lib/components/app-state/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { LoadingButton, PageState } from '#lib/components/app-state/index.js';
	import CareSection from './CareSection.svelte';
	import CareSelect from './CareSelect.svelte';
	import CareStaffPicker from './CareStaffPicker.svelte';
	import CareStudentPicker from './CareStudentPicker.svelte';
	let {
		initial,
		students,
		staff,
		term,
		date
	}: {
		initial?: CareVisitRecord;
		students: CareStudentOption[];
		staff: CareStaffOption[];
		term: string;
		date: string;
	} = $props();
	let record = $state(untrack(() => initial)),
		selected = $state<CareStudentOption[]>([]),
		visitorIds = $state(untrack(() => initial?.visit.visitorIds ?? [])),
		plannedDate = $state(untrack(() => initial?.visit.plannedDate ?? date)),
		visitedDate = $state(untrack(() => initial?.visitedDate ?? '')),
		contact = $state(untrack(() => initial?.contactPerson ?? '')),
		observations = $state(untrack(() => initial?.factualObservations ?? '')),
		strengths = $state(untrack(() => initial?.strengths ?? '')),
		needs = $state(untrack(() => initial?.needs ?? '')),
		actions = $state(untrack(() => initial?.agreedActions ?? '')),
		nextFollowUp = $state(untrack(() => initial?.nextFollowUp ?? '')),
		status = $state<CareVisitStatus>(untrack(() => initial?.visit.status ?? 'planned')),
		pending = $state(false),
		error = $state(''),
		message = $state('');
	let roster = $state(untrack(() => students)),
		rosterPending = $state(false),
		rosterError = $state('');
	const rosterRequest = new LatestRequest();
	const editable = $derived(
		record
			? record.visit.status !== 'cancelled' && mayManageCareVisit(record.visit)
			: $can.hasAny(...careManagePermissions)
	);
	async function loadRoster(value: string) {
		plannedDate = value;
		if (record) return;
		selected = [];
		const token = rosterRequest.begin();
		const identity = $careIdentity;
		rosterPending = true;
		rosterError = '';
		try {
			const rows = await careStudents(
				{ academicTermId: term, date: value, limit: 50 },
				{ signal: token.signal }
			);
			if (rosterRequest.isCurrent(token.revision) && identity === $careIdentity) roster = rows;
		} catch (cause) {
			if (rosterRequest.isCurrent(token.revision) && identity === $careIdentity)
				rosterError = cause instanceof Error ? cause.message : 'โหลดรายชื่อไม่ได้';
		} finally {
			if (rosterRequest.isCurrent(token.revision) && identity === $careIdentity)
				rosterPending = false;
		}
	}
	let alive = true;
	onDestroy(() => {
		alive = false;
		rosterRequest.abort();
	});
	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (pending || !editable || rosterPending || rosterError) return;
		const studentId = record?.visit.studentId ?? selected[0]?.id;
		if (!studentId || !visitorIds.length || !plannedDate) {
			error = 'เลือกนักเรียน วันที่นัดหมาย และผู้เยี่ยมบ้าน';
			return;
		}
		if (
			status === 'completed' &&
			(!visitedDate || !contact.trim() || !observations.trim() || !actions.trim())
		) {
			error =
				'ระบุวันที่เยี่ยมจริง ผู้ให้ข้อมูล ข้อเท็จจริง และแนวทางที่ตกลงก่อนบันทึกว่าเยี่ยมแล้ว';
			return;
		}
		if (nextFollowUp && nextFollowUp < (visitedDate || plannedDate)) {
			error = 'วันติดตามต้องไม่ก่อนวันที่เยี่ยม';
			return;
		}
		if (
			!$can.has(PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL) &&
			$authStore.user &&
			!visitorIds.includes($authStore.user.id)
		) {
			error = 'ผู้บันทึกต้องอยู่ในรายชื่อผู้เยี่ยมบ้านด้วย';
			return;
		}
		pending = true;
		error = '';
		message = '';
		const identity = $careIdentity;
		try {
			const next = await saveCareVisit(
				mutation.prepare(
					{
						requestId: crypto.randomUUID(),
						rowVersion: record?.visit.rowVersion ?? null,
						studentId,
						academicTermId: record?.visit.academicTermId ?? term,
						plannedDate,
						visitedDate: visitedDate || null,
						contactPerson: contact.trim(),
						factualObservations: observations.trim(),
						visitorIds,
						status,
						strengths: strengths.trim(),
						needs: needs.trim(),
						agreedActions: actions.trim(),
						nextFollowUp: nextFollowUp || null
					},
					'saveCareVisit'
				),
				record?.visit.id
			);
			if (alive && identity === $careIdentity) {
				record = next;
				message = 'บันทึกเยี่ยมบ้านแล้ว';
				if (!initial) await goto(careLink(`/visits/${next.visit.id}`, term));
			}
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
		title="1. แผนเยี่ยมบ้าน"
		icon={House}
		description={record?.visit.studentName ?? 'เลือกนักเรียน วันนัดหมาย และบุคลากรที่จะเยี่ยมบ้าน'}
	>
		{#if !record && rosterPending}<PageSkeleton
				variant="detail"
			/>{:else if !record && rosterError}<PageState
				variant="error"
				title="โหลดรายชื่อวันที่เลือกไม่ได้"
				description={rosterError}
				actionLabel="ลองโหลดรายชื่อใหม่"
				onaction={() => loadRoster(plannedDate)}
			/>{:else if !record}{#key plannedDate}<CareStudentPicker
					initial={roster}
					{term}
					date={plannedDate}
					value={selected}
					onchange={(value) => (selected = value)}
				/>{/key}{/if}
		<div class="grid gap-4 sm:grid-cols-2">
			<div class="space-y-2">
				<Label for="visit-planned">วันที่นัดหมาย *</Label><DatePicker
					id="visit-planned"
					ariaLabel="วันที่นัดหมาย"
					value={plannedDate}
					onValueChange={(value) => {
						if (value) void loadRoster(value);
					}}
				/>
			</div>
			<CareSelect
				id="visit-status"
				label="สถานะการเยี่ยมบ้าน"
				value={status}
				options={Object.entries(visitLabels)
					.filter(([value]) => !!record || value !== 'cancelled')
					.map(([value, label]) => ({ value, label }))}
				onValueChange={(value) => {
					if (value === 'planned' || value === 'completed' || value === 'cancelled') status = value;
				}}
			/>
		</div>
		<CareStaffPicker
			{staff}
			value={visitorIds}
			onchange={(value) => (visitorIds = value)}
			label="ผู้เยี่ยมบ้าน"
			disabled={!!record && !$can.has(PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL)}
		/>
	</CareSection><CareSection
		title="2. บันทึกการเยี่ยมบ้าน"
		icon={NotebookPen}
		description={status === 'completed'
			? 'เมื่อเลือกเยี่ยมแล้ว ต้องระบุวันที่เยี่ยมจริง ผู้ให้ข้อมูล ข้อเท็จจริง และแนวทางที่ตกลงร่วมกัน'
			: 'ส่วนนี้ยังไม่จำเป็นสำหรับการวางแผน คุณสามารถจดข้อมูลที่ทราบแล้วและกลับมาเติมหลังเยี่ยมบ้าน'}
	>
		<div class="grid gap-4 sm:grid-cols-2">
			<div class="space-y-2">
				<Label for="visit-actual"
					>วันที่เยี่ยมจริง{#if status === 'completed'}<span aria-hidden="true">
							*</span
						>{/if}</Label
				><DatePicker
					id="visit-actual"
					required={status === 'completed'}
					ariaLabel="วันที่เยี่ยมจริง"
					value={visitedDate}
					onValueChange={(value) => (visitedDate = value ?? '')}
				/>
			</div>
			<div class="space-y-2">
				<Label for="visit-contact"
					>ผู้ให้ข้อมูล / ความสัมพันธ์{#if status === 'completed'}<span aria-hidden="true">
							*</span
						>{/if}</Label
				><Input
					id="visit-contact"
					aria-required={status === 'completed'}
					bind:value={contact}
					maxlength={500}
				/>
			</div>
		</div>
		<div class="space-y-2">
			<Label for="visit-observations"
				>ข้อเท็จจริงที่สังเกต{#if status === 'completed'}<span aria-hidden="true">
						*</span
					>{/if}</Label
			><Textarea
				id="visit-observations"
				aria-required={status === 'completed'}
				bind:value={observations}
				maxlength={5000}
			/>
		</div>
		<div class="space-y-2">
			<Label for="visit-strengths">จุดแข็งและสิ่งสนับสนุน</Label><Textarea
				id="visit-strengths"
				bind:value={strengths}
				maxlength={5000}
			/>
		</div>
		<div class="space-y-2">
			<Label for="visit-needs">ความต้องการการช่วยเหลือ</Label><Textarea
				id="visit-needs"
				bind:value={needs}
				maxlength={5000}
			/>
		</div>
		<div class="space-y-2">
			<Label for="visit-actions"
				>แนวทางที่ตกลงร่วมกัน{#if status === 'completed'}<span aria-hidden="true">
						*</span
					>{/if}</Label
			><Textarea
				id="visit-actions"
				aria-required={status === 'completed'}
				bind:value={actions}
				maxlength={5000}
			/>
		</div>
		<div class="space-y-2">
			<Label for="visit-followup">นัดติดตามครั้งต่อไป</Label><DatePicker
				id="visit-followup"
				ariaLabel="นัดติดตามครั้งต่อไป"
				value={nextFollowUp}
				onValueChange={(value) => (nextFollowUp = value ?? '')}
				clearable
			/>
		</div>
	</CareSection>{#if error}<PageState
			variant="error"
			title="บันทึกเยี่ยมบ้านไม่ได้"
			description={error}
		/>{/if}{#if message}<p role="status" class="text-sm text-success">{message}</p>{/if}
	<div class="flex justify-end gap-2">
		<Button variant="outline" href={careLink('/visits', term)}>กลับรายการ</Button><LoadingButton
			type="submit"
			loading={pending}
			disabled={!editable || rosterPending || !!rosterError}
			><Save aria-hidden="true" />บันทึกเยี่ยมบ้าน</LoadingButton
		>
	</div>
</form>
