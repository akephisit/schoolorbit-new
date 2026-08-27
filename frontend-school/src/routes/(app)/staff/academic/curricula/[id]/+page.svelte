<script lang="ts">
	import { afterNavigate, goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		createCurriculumVersion,
		createStudyProgram,
		getCurriculum,
		getCurriculumManagementOptions,
		getCurriculumProgramWorkspace,
		getStudyProgram,
		listAcademicYears,
		listCurriculumVersions,
		publishCurriculumVersion,
		replaceProgramRequirements,
		type AcademicYear,
		type CreateCurriculumVersionRequest,
		type CreateStudyProgramRequest,
		type Curriculum,
		type CurriculumManagementOptions,
		type CurriculumProgramWorkspace,
		type CurriculumRequirementView,
		type CurriculumVersion,
		type ProgramRequirement,
		type ProgramRequirementInput,
		type StudyProgram
	} from '$lib/api/academic-core';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import CurriculumProgramEditor from '$lib/components/academic-core/CurriculumProgramEditor.svelte';
	import CurriculumVersionPanel from '$lib/components/academic-core/CurriculumVersionPanel.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { ArrowLeft } from 'lucide-svelte';

	const detailRequest = new LatestRequest();
	const versionRequest = new LatestRequest();
	const managementCache = new SvelteMap<string, CurriculumManagementOptions>();

	let curriculum = $state.raw<Curriculum | null>(null);
	let versions = $state.raw<CurriculumVersion[]>([]);
	let selectedVersion = $state.raw<CurriculumVersion | null>(null);
	let workspace = $state.raw<CurriculumProgramWorkspace>({ programs: [], requirements: [] });
	let academicYears = $state.raw<AcademicYear[]>([]);
	let loading = $state(true);
	let workspaceLoading = $state(false);
	let initialized = $state(false);
	let errorMessage = $state('');
	let workspaceError = $state('');
	let curriculumId = $derived(page.params.id ?? '');
	let canManageAcademicCurriculum = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT
		)
	);
	let selectedManagementOptions = $derived(
		selectedVersion ? (managementCache.get(selectedVersion.id) ?? null) : null
	);

	async function loadDetail() {
		const { revision, signal } = detailRequest.begin();
		loading = true;
		errorMessage = '';
		try {
			const loadedCurriculum = await getCurriculum(curriculumId, { signal });
			const loadedVersions = await listCurriculumVersions(curriculumId, { signal });
			const loadedYears = await listAcademicYears({ signal });
			const requestedVersionId = page.url.searchParams.get('versionId');
			const version =
				loadedVersions.find((candidate) => candidate.id === requestedVersionId) ??
				loadedVersions[0] ??
				null;
			const loadedWorkspace = version
				? await getCurriculumProgramWorkspace(version.id, { signal })
				: { programs: [], requirements: [] };
			if (!detailRequest.isCurrent(revision)) return;
			curriculum = loadedCurriculum;
			versions = loadedVersions;
			academicYears = loadedYears;
			selectedVersion = version;
			workspace = loadedWorkspace;
			initialized = true;
			if (version && requestedVersionId !== version.id) {
				await goto(
					resolve(
						`/staff/academic/curricula/${curriculumId}?versionId=${encodeURIComponent(version.id)}`
					),
					{ replaceState: true, keepFocus: true, noScroll: true }
				);
			}
		} catch (error) {
			if (isAbortError(error)) return;
			if (detailRequest.isCurrent(revision)) {
				errorMessage = error instanceof Error ? error.message : 'โหลดรายละเอียดหลักสูตรไม่สำเร็จ';
			}
		} finally {
			if (detailRequest.isCurrent(revision)) loading = false;
		}
	}

	async function loadVersion(version: CurriculumVersion, updateUrl = true) {
		const { revision, signal } = versionRequest.begin();
		workspaceLoading = true;
		workspaceError = '';
		try {
			const loadedWorkspace = await getCurriculumProgramWorkspace(version.id, { signal });
			if (!versionRequest.isCurrent(revision)) return;
			selectedVersion = version;
			workspace = loadedWorkspace;
			if (updateUrl) {
				await goto(
					resolve(
						`/staff/academic/curricula/${curriculumId}?versionId=${encodeURIComponent(version.id)}`
					),
					{ keepFocus: true, noScroll: true }
				);
			}
		} catch (error) {
			if (isAbortError(error)) return;
			if (versionRequest.isCurrent(revision)) {
				workspaceError = error instanceof Error ? error.message : 'โหลดรุ่นหลักสูตรไม่สำเร็จ';
			}
		} finally {
			if (versionRequest.isCurrent(revision)) workspaceLoading = false;
		}
	}

	async function requestManagementOptions() {
		if (!canManageAcademicCurriculum || !selectedVersion) return null;
		const cached = managementCache.get(selectedVersion.id);
		if (cached) return cached;
		const loaded = await getCurriculumManagementOptions(selectedVersion.id);
		managementCache.set(selectedVersion.id, loaded);
		return loaded;
	}

	async function createVersion(draft: CreateCurriculumVersionRequest) {
		const created = await createCurriculumVersion(curriculumId, draft);
		versions = [created, ...versions];
		await loadVersion(created);
	}

	async function createProgram(draft: CreateStudyProgramRequest) {
		if (!selectedVersion) return;
		const created = await createStudyProgram(selectedVersion.id, draft);
		workspace = {
			...workspace,
			programs: [...workspace.programs, created]
		};
	}

	function resolveRequirementViews(
		programId: string,
		updated: ProgramRequirement[],
		options: CurriculumManagementOptions
	): CurriculumRequirementView[] {
		return updated.map((requirement) => {
			const gradeLevel = options.gradeLevels.find(
				(option) => option.id === requirement.gradeLevelId
			);
			const catalog = options.catalogVersions.find(
				(option) => option.id === requirement.catalogVersionId
			);
			if (!gradeLevel || !catalog) {
				throw new Error(
					'บันทึกสำเร็จแต่ไม่สามารถจับคู่ชื่อระดับชั้นหรือรายการทะเบียนได้ กรุณาโหลดหน้าใหม่'
				);
			}
			return { studyProgramId: programId, requirement, gradeLevel, catalog };
		});
	}

	async function replaceRequirements(
		program: StudyProgram,
		requirements: ProgramRequirementInput[]
	) {
		const options = await requestManagementOptions();
		if (!options) throw new Error('ไม่มีสิทธิ์จัดการรายการในแผนการเรียน');
		const updated = await replaceProgramRequirements(program.id, {
			rowVersion: program.rowVersion,
			requirements
		});
		const views = resolveRequirementViews(program.id, updated, options);
		const refreshedProgram = await getStudyProgram(program.id);
		workspace = {
			programs: workspace.programs.map((item) =>
				item.id === refreshedProgram.id ? refreshedProgram : item
			),
			requirements: [
				...workspace.requirements.filter((item) => item.studyProgramId !== program.id),
				...views
			]
		};
	}

	async function publishVersion(id: string, rowVersion: number) {
		const updated = await publishCurriculumVersion(id, { rowVersion });
		versions = versions.map((version) => (version.id === updated.id ? updated : version));
		selectedVersion = updated;
	}

	afterNavigate(({ to }) => {
		const requestedVersionId = to?.url.searchParams.get('versionId') ?? null;
		if (!initialized || versions.length === 0) return;
		const target =
			versions.find((version) => version.id === requestedVersionId) ?? versions[0] ?? null;
		if (target && target.id !== selectedVersion?.id) void loadVersion(target, false);
	});

	onMount(() => {
		void loadDetail();
		return () => {
			detailRequest.abort();
			versionRequest.abort();
		};
	});
</script>

<PageShell
	title={curriculum?.nameTh ?? 'รายละเอียดหลักสูตร'}
	description="จัดรุ่น แผนการเรียน และรายการรายวิชาหรือกิจกรรมด้วยชื่อที่อ่านเข้าใจได้"
>
	{#snippet actions()}
		<Button href="/staff/academic/curricula" variant="outline">
			<ArrowLeft class="size-4" /> กลับภาพรวม
		</Button>
	{/snippet}

	{#if loading}
		<PageSkeleton variant="cards" rows={5} />
	{:else if errorMessage || !curriculum}
		<PageState
			variant="error"
			title="โหลดรายละเอียดหลักสูตรไม่สำเร็จ"
			description={errorMessage || 'ไม่พบหลักสูตร'}
			actionLabel="ลองอีกครั้ง"
			onaction={loadDetail}
		/>
	{:else}
		<div class="space-y-5">
			<CurriculumVersionPanel
				{curriculum}
				{versions}
				{selectedVersion}
				{academicYears}
				canManage={canManageAcademicCurriculum}
				onSelectVersion={loadVersion}
				onRequestManagementOptions={requestManagementOptions}
				onCreateVersion={createVersion}
			/>

			{#if workspaceLoading}
				<PageSkeleton variant="cards" rows={4} />
			{:else if workspaceError}
				<PageState
					variant="error"
					title="โหลดแผนการเรียนไม่สำเร็จ"
					description={workspaceError}
					actionLabel="ลองอีกครั้ง"
					onaction={() => selectedVersion && loadVersion(selectedVersion, false)}
				/>
			{:else if selectedVersion}
				{#key selectedVersion.id}
					<CurriculumProgramEditor
						version={selectedVersion}
						programs={workspace.programs}
						requirements={workspace.requirements}
						managementOptions={selectedManagementOptions}
						canManage={canManageAcademicCurriculum}
						onRequestManagementOptions={requestManagementOptions}
						onCreateProgram={createProgram}
						onReplaceRequirements={replaceRequirements}
						onPublishVersion={publishVersion}
					/>
				{/key}
			{:else}
				<PageState
					title="ยังไม่มีรุ่นหลักสูตร"
					description="สร้างรุ่นแบบร่างเพื่อเริ่มกำหนดแผนการเรียนและรายการในแผน"
				/>
			{/if}
		</div>
	{/if}
</PageShell>
