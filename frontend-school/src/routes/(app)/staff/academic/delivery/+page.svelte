<script lang="ts">
	import { onMount } from 'svelte';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import { listHomerooms, listStaffOptions } from '$lib/api/academic-core';
	import {
		applyLearningGroupRoster,
		applyLearningOfferingsFromCurriculum,
		createLearningGroup,
		createLearningOffering,
		listLearningGroupsForTerm,
		listLearningOfferings,
		previewLearningGroupRoster,
		previewLearningOfferingsFromCurriculum,
		publishLearningGroupRoster,
		publishLearningOffering,
		replaceLearningGroupHomerooms,
		replaceLearningGroupTeachers,
		updateLearningGroup,
		type CurriculumOfferingPreview,
		type LearningGroup,
		type LearningOffering,
		type RosterPreview
	} from '$lib/api/learning-delivery';
	import CurriculumOfferingPreviewPanel from '$lib/components/learning-delivery/CurriculumOfferingPreview.svelte';
	import LearningGroupEditor from '$lib/components/learning-delivery/LearningGroupEditor.svelte';
	import LearningOfferingEditor from '$lib/components/learning-delivery/LearningOfferingEditor.svelte';
	import RosterPreviewPanel from '$lib/components/learning-delivery/RosterPreviewPanel.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import {
		AcademicPrerequisiteNotice,
		type AcademicPrerequisite
	} from '$lib/components/academic-workflow';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	const academicContext = getAcademicContextStore();
	const academicTermId = $derived($academicContext.selected.academicTermId);
	const academicYearId = $derived($academicContext.selected.academicYearId);
	let offerings = $state<LearningOffering[]>([]);
	let selectedOffering = $state<LearningOffering | null>(null);
	let allGroups = $state<LearningGroup[]>([]);
	let groups = $state<LearningGroup[]>([]);
	let selectedGroup = $state<LearningGroup | null>(null);
	let rosterPreview = $state<RosterPreview | null>(null);
	let curriculumPreview = $state<CurriculumOfferingPreview | null>(null);
	let homeroomOptions = $state<Array<{ id: string; name: string }>>([]);
	let staffOptions = $state<Array<{ id: string; name: string }>>([]);
	let loading = $state(false);
	let rosterLoading = $state(false);
	let errorMessage = $state('');
	let revision = 0;
	const missingTermPrerequisite: AcademicPrerequisite = {
		key: 'academic-term',
		status: 'missing',
		title: 'เลือกปีการศึกษาและภาคเรียนก่อน',
		description: 'รายการเปิดสอน กลุ่มเรียน และรายชื่อนักเรียนแยกกันในแต่ละภาคเรียน',
		actionLabel: 'ไปตั้งค่าปีและภาคเรียน',
		href: '/staff/academic/core'
	};
	const noOfferingPrerequisite: AcademicPrerequisite = {
		key: 'learning-offerings',
		status: 'warning',
		title: 'ภาคเรียนนี้ยังไม่มีรายการเปิดสอน',
		description: 'ตรวจหลักสูตรก่อนสร้างรายการเปิดสอน แล้วจึงจัดกลุ่มเรียน ครู ห้อง และรายชื่อ',
		actionLabel: 'ตรวจหลักสูตร',
		href: '/staff/academic/curricula'
	};
	const canManage = $derived(
		$can.hasAny(
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ASSIGNED
		)
	);

	async function loadWorkspace(termId: string, yearId: string) {
		const current = ++revision;
		loading = true;
		errorMessage = '';
		try {
			const [rows, termGroups, rooms, staff] = await Promise.all([
				listLearningOfferings(termId),
				listLearningGroupsForTerm(termId),
				listHomerooms(yearId),
				listStaffOptions()
			]);
			if (current !== revision) return;
			offerings = rows;
			allGroups = termGroups;
			homeroomOptions = rooms.map((room) => ({ id: room.id, name: room.name }));
			staffOptions = staff.map((person) => ({ id: person.id, name: person.name }));
			selectedOffering = null;
			selectedGroup = null;
			groups = [];
			rosterPreview = null;
			curriculumPreview = null;
		} catch (error) {
			if (current === revision)
				errorMessage = error instanceof Error ? error.message : 'โหลดรายการเปิดสอนไม่สำเร็จ';
		} finally {
			if (current === revision) loading = false;
		}
	}
	function selectOffering(offering: LearningOffering) {
		selectedOffering = offering;
		selectedGroup = null;
		rosterPreview = null;
		groups = allGroups.filter((group) => group.learningOfferingId === offering.id);
	}
	function replaceGroup(updated: LearningGroup) {
		allGroups = allGroups.map((group) => (group.id === updated.id ? updated : group));
		groups = groups.map((group) => (group.id === updated.id ? updated : group));
		if (selectedGroup?.id === updated.id) selectedGroup = updated;
	}
	async function addOffering(draft: {
		kind: 'course' | 'activity';
		catalogVersionId: string;
		owningOrganizationUnitId: string;
		gradeLevelId: string;
		studyProgramId: string;
	}) {
		if (!academicTermId) throw new Error('กรุณาเลือกภาคเรียนก่อน');
		const targets = [
			{
				gradeLevelId: draft.gradeLevelId,
				studyProgramId: draft.studyProgramId,
				homeroomId: null,
				targetKind: 'grade_program' as const
			}
		];
		const body =
			draft.kind === 'course'
				? {
						kind: 'course' as const,
						academicTermId,
						subjectVersionId: draft.catalogVersionId,
						curriculumCourseRequirementId: null,
						owningOrganizationUnitId: draft.owningOrganizationUnitId,
						gradingPolicy: { policyCode: 'school_default' },
						targets
					}
				: {
						kind: 'activity' as const,
						academicTermId,
						activityVersionId: draft.catalogVersionId,
						curriculumActivityRequirementId: null,
						owningOrganizationUnitId: draft.owningOrganizationUnitId,
						attendanceRequirement: { minimumPercent: null, requiredSessions: null },
						passCriteria: {
							outcomes: ['ผ่าน'],
							requireAttendance: true,
							requireTeacherConfirmation: true
						},
						registrationType: 'assigned' as const,
						schedulingMode: 'synchronized' as const,
						capacity: null,
						targets
					};
		offerings = [...offerings, await createLearningOffering(body)];
	}
	async function publishOffering(offering: LearningOffering) {
		const updated = await publishLearningOffering(offering.id, {
			rowVersion: offering.rowVersion,
			idempotencyKey: crypto.randomUUID()
		});
		offerings = offerings.map((item) => (item.id === updated.id ? updated : item));
		if (selectedOffering?.id === updated.id) selectedOffering = updated;
	}
	async function addGroup(draft: {
		code: string;
		name: string;
		description: string;
		capacity: number | null;
		preferredRoomIds: string[];
	}) {
		if (!selectedOffering) return;
		const created = await createLearningGroup(selectedOffering.id, {
			...draft,
			description: draft.description || null
		});
		allGroups = [...allGroups, created];
		groups = [...groups, created];
	}
	async function configureGroup(
		group: LearningGroup,
		draft: {
			homeroomIds: string[];
			preferredRoomIds: string[];
			teacherId: string;
			teacherRole: 'primary' | 'secondary' | 'assistant';
		}
	) {
		const details = await updateLearningGroup(group.id, {
			code: group.code,
			name: group.name,
			description: group.description ?? null,
			capacity: group.capacity ?? null,
			preferredRoomIds: draft.preferredRoomIds,
			rowVersion: group.rowVersion
		});
		const withHomerooms = await replaceLearningGroupHomerooms(group.id, {
			homeroomIds: draft.homeroomIds,
			rowVersion: details.rowVersion
		});
		const teacherAssignments = withHomerooms.teacherAssignments.filter(
			(teacher) => teacher.teacherId !== draft.teacherId
		);
		const updated = await replaceLearningGroupTeachers(group.id, {
			rowVersion: withHomerooms.rowVersion,
			teachers: [...teacherAssignments, { teacherId: draft.teacherId, role: draft.teacherRole }]
		});
		replaceGroup(updated);
	}
	async function refreshRoster() {
		if (!selectedGroup) return;
		rosterLoading = true;
		try {
			rosterPreview = await previewLearningGroupRoster(selectedGroup.id);
		} finally {
			rosterLoading = false;
		}
	}
	async function applyRoster(sourceHash: string) {
		if (!selectedGroup) return;
		const updated = await applyLearningGroupRoster(selectedGroup.id, {
			sourceHash,
			rowVersion: selectedGroup.rowVersion,
			overrides: []
		});
		replaceGroup(updated);
	}
	async function publishRoster() {
		if (!selectedGroup) return;
		const updated = await publishLearningGroupRoster(selectedGroup.id, {
			rowVersion: selectedGroup.rowVersion,
			idempotencyKey: crypto.randomUUID()
		});
		replaceGroup(updated);
	}
	async function buildCurriculumPreview(draft: {
		studyProgramIds: string[];
		owningOrganizationUnitId: string;
	}) {
		if (!academicTermId) return;
		curriculumPreview = await previewLearningOfferingsFromCurriculum({
			academicTermId,
			studyProgramIds: draft.studyProgramIds
		});
	}
	async function applyCurriculumPreview(
		sourceHash: string,
		studyProgramIds: string[],
		owningOrganizationUnitId: string
	) {
		if (!academicTermId || !academicYearId) return;
		await applyLearningOfferingsFromCurriculum({
			academicTermId,
			sourceHash,
			studyProgramIds,
			owningOrganizationUnitId,
			idempotencyKey: crypto.randomUUID()
		});
		await loadWorkspace(academicTermId, academicYearId);
	}

	onMount(() => {
		let loadedTermId: string | null = null;
		return academicContext.subscribe((state) => {
			const termId = state.selected.academicTermId;
			const yearId = state.selected.academicYearId;
			if (termId && yearId && termId !== loadedTermId) {
				loadedTermId = termId;
				void loadWorkspace(termId, yearId);
			}
		});
	});
</script>

<PageShell
	title="รายวิชาและกิจกรรมที่เปิดสอน"
	description="นำรายการจากหลักสูตรมาเปิดสอน แล้วจัดกลุ่ม ครู ห้อง และรายชื่อนักเรียนของภาคเรียน"
>
	{#if !academicTermId || !academicYearId}
		<AcademicPrerequisiteNotice prerequisite={missingTermPrerequisite} />
	{:else if loading}<PageSkeleton variant="cards" rows={5} />{:else if errorMessage}<PageState
			variant="error"
			title="โหลดรายการเปิดสอนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadWorkspace(academicTermId, academicYearId)}
		/>{:else}
		<CurriculumOfferingPreviewPanel
			preview={curriculumPreview}
			{canManage}
			onPreview={buildCurriculumPreview}
			onApply={applyCurriculumPreview}
		/>
		{#if offerings.length === 0}
			<AcademicPrerequisiteNotice prerequisite={noOfferingPrerequisite} />
		{/if}
		<LearningOfferingEditor
			{offerings}
			{canManage}
			onSelect={selectOffering}
			onCreate={addOffering}
			onPublish={publishOffering}
		/>
		{#if selectedOffering}<LearningGroupEditor
				offering={selectedOffering}
				{groups}
				{homeroomOptions}
				{staffOptions}
				{canManage}
				onCreate={addGroup}
				onConfigure={configureGroup}
				onPreviewRoster={(group) => {
					selectedGroup = group;
					rosterPreview = null;
					void refreshRoster();
				}}
			/>{/if}
		{#if selectedGroup}<RosterPreviewPanel
				group={selectedGroup}
				preview={rosterPreview}
				loading={rosterLoading}
				{canManage}
				onRefresh={refreshRoster}
				onApply={applyRoster}
				onPublish={publishRoster}
			/>{/if}
	{/if}
</PageShell>
