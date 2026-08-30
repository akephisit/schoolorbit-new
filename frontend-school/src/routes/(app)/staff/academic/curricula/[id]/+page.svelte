<script lang="ts">
	import { afterNavigate, goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import { buildCurriculumValidationNoticeViews } from '$lib/academic/curriculum-structure';
	import {
		cloneCurriculumVersionDraft,
		createCurriculumVersion,
		createStudyProgram,
		getCurriculum,
		getCurriculumCreateOptions,
		getCurriculumManagementOptions,
		getCurriculumStructureWorkspace,
		listCurriculumVersions,
		publishCurriculumVersion,
		replaceCurriculumStructure,
		replaceCurriculumTermSlots,
		type CreateCurriculumVersionRequest,
		type CloneCurriculumVersionRequest,
		type CreateStudyProgramRequest,
		type Curriculum,
		type CurriculumCreateOptions,
		type CurriculumManagementOptions,
		type CurriculumStructureRequirementInput,
		type CurriculumStructureWorkspace,
		type CurriculumTermSlotInput,
		type CurriculumVersionView
	} from '$lib/api/academic-core';
	import {
		getHomeroomDeliveryWorkspace,
		type HomeroomDeliveryWorkspace
	} from '$lib/api/learning-delivery';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import CurriculumProgramComparison from '$lib/components/academic-core/CurriculumProgramComparison.svelte';
	import CurriculumDeliveryAlignmentPanel from '$lib/components/academic-core/CurriculumDeliveryAlignmentPanel.svelte';
	import CurriculumStructureEditor from '$lib/components/academic-core/CurriculumStructureEditor.svelte';
	import CurriculumStructureToolbar from '$lib/components/academic-core/CurriculumStructureToolbar.svelte';
	import CurriculumTermDocument from '$lib/components/academic-core/CurriculumTermDocument.svelte';
	import CurriculumVersionPanel from '$lib/components/academic-core/CurriculumVersionPanel.svelte';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { ArrowLeft } from 'lucide-svelte';

	const detailRequest = new LatestRequest();
	const versionRequest = new LatestRequest();
	const alignmentRequest = new LatestRequest();
	const managementCache = new SvelteMap<string, CurriculumManagementOptions>();

	let curriculum = $state.raw<Curriculum | null>(null);
	let versions = $state.raw<CurriculumVersionView[]>([]);
	let selectedVersion = $state.raw<CurriculumVersionView | null>(null);
	let workspace = $state.raw<CurriculumStructureWorkspace | null>(null);
	let createOptions = $state.raw<CurriculumCreateOptions | null>(null);
	let alignmentWorkspace = $state.raw<HomeroomDeliveryWorkspace | null>(null);
	let loading = $state(true);
	let workspaceLoading = $state(false);
	let alignmentLoading = $state(false);
	let initialized = $state(false);
	let errorMessage = $state('');
	let workspaceError = $state('');
	let alignmentError = $state('');
	let editorOpen = $state(false);
	let viewMode = $state<'comparison' | 'document'>('comparison');
	let selectedGradeLevelId = $state('');
	let selectedStudyProgramId = $state('');
	let loadedAlignmentContextKey = '';
	let curriculumId = $derived(page.params.id ?? '');
	let deliveryContext = $derived(readDeliveryContext(page.url));
	let canManageAcademicCurriculum = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT
		)
	);
	let selectedManagementOptions = $derived(
		selectedVersion ? (managementCache.get(selectedVersion.version.id) ?? null) : null
	);
	let validationBlockers = $derived(
		buildCurriculumValidationNoticeViews(workspace?.validation.blockers ?? [])
	);

	function readDeliveryContext(url: URL) {
		const academicYearId = url.searchParams.get('academicYearId')?.trim() ?? '';
		const academicTermId = url.searchParams.get('academicTermId')?.trim() ?? '';
		if (!academicYearId || !academicTermId) return null;
		return {
			academicYearId,
			academicTermId,
			studyProgramId: url.searchParams.get('studyProgramId')?.trim() || undefined,
			timetableVersionId: url.searchParams.get('timetableVersionId')?.trim() || undefined
		};
	}

	function deliveryContextKey(url: URL): string {
		const context = readDeliveryContext(url);
		return context
			? `${context.academicYearId}:${context.academicTermId}:${context.studyProgramId ?? ''}:${context.timetableVersionId ?? ''}`
			: '';
	}

	function curriculumVersionUrl(
		versionId: string,
		currentUrl: URL = page.url
	): `/staff/academic/curricula/${string}?${string}` {
		const query = new URLSearchParams(currentUrl.searchParams);
		query.set('versionId', versionId);
		return `/staff/academic/curricula/${encodeURIComponent(curriculumId)}?${query.toString()}`;
	}

	function applyWorkspace(value: CurriculumStructureWorkspace | null) {
		workspace = value;
		if (!value) return;
		if (!value.gradeLevels.some((grade) => grade.id === selectedGradeLevelId)) {
			selectedGradeLevelId = value.gradeLevels[0]?.id ?? '';
		}
		if (!value.programs.some((program) => program.id === selectedStudyProgramId)) {
			selectedStudyProgramId = value.programs[0]?.id ?? '';
		}
	}

	async function loadDetail() {
		const { revision, signal } = detailRequest.begin();
		loading = true;
		errorMessage = '';
		try {
			const loadedCurriculum = await getCurriculum(curriculumId, { signal });
			const loadedVersions = await listCurriculumVersions(curriculumId, { signal });
			const requestedVersionId = page.url.searchParams.get('versionId');
			const version =
				loadedVersions.find((candidate) => candidate.version.id === requestedVersionId) ??
				loadedVersions[0] ??
				null;
			const loadedWorkspace = version
				? await getCurriculumStructureWorkspace(version.version.id, { signal })
				: null;
			if (!detailRequest.isCurrent(revision)) return;
			curriculum = loadedCurriculum;
			versions = loadedVersions;
			selectedVersion = version;
			applyWorkspace(loadedWorkspace);
			initialized = true;
			if (version && requestedVersionId !== version.version.id) {
				await goto(
					resolve(curriculumVersionUrl(version.version.id)),
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

	async function loadVersion(version: CurriculumVersionView, updateUrl = true) {
		const { revision, signal } = versionRequest.begin();
		workspaceLoading = true;
		workspaceError = '';
		try {
			const loadedWorkspace = await getCurriculumStructureWorkspace(version.version.id, { signal });
			if (!versionRequest.isCurrent(revision)) return;
			selectedVersion = version;
			applyWorkspace(loadedWorkspace);
			if (updateUrl) {
				await goto(
					resolve(curriculumVersionUrl(version.version.id)),
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

	async function loadAlignment(url: URL = page.url) {
		const context = readDeliveryContext(url);
		if (!context) {
			alignmentRequest.abort();
			alignmentWorkspace = null;
			alignmentError = '';
			alignmentLoading = false;
			return;
		}
		const { revision, signal } = alignmentRequest.begin();
		alignmentLoading = true;
		alignmentError = '';
		try {
			const loaded = await getHomeroomDeliveryWorkspace(
				context.academicYearId,
				context.academicTermId,
				{ signal, timetableVersionId: context.timetableVersionId }
			);
			if (alignmentRequest.isCurrent(revision)) alignmentWorkspace = loaded;
		} catch (error) {
			if (isAbortError(error)) return;
			if (alignmentRequest.isCurrent(revision)) {
				alignmentError =
					error instanceof Error ? error.message : 'โหลดข้อมูลเทียบการเปิดสอนไม่สำเร็จ';
			}
		} finally {
			if (alignmentRequest.isCurrent(revision)) alignmentLoading = false;
		}
	}

	async function requestManagementOptions() {
		if (!canManageAcademicCurriculum || !selectedVersion) return null;
		const versionId = selectedVersion.version.id;
		const cached = managementCache.get(versionId);
		if (cached) return cached;
		const loaded = await getCurriculumManagementOptions(versionId);
		managementCache.set(versionId, loaded);
		return loaded;
	}

	async function requestCreateOptions() {
		if (!canManageAcademicCurriculum) return null;
		if (createOptions) return createOptions;
		createOptions = await getCurriculumCreateOptions();
		return createOptions;
	}

	async function createVersion(draft: CreateCurriculumVersionRequest) {
		const options = await requestCreateOptions();
		if (!options) throw new Error('ไม่มีสิทธิ์สร้างรุ่นหลักสูตร');
		const created = await createCurriculumVersion(curriculumId, draft);
		const start = options.academicYears.find((year) => year.id === created.startAcademicYearId);
		const end = options.academicYears.find((year) => year.id === created.endAcademicYearId);
		if (!start) throw new Error('สร้างรุ่นสำเร็จแต่ไม่พบชื่อปีเริ่มใช้ กรุณาโหลดหน้าใหม่');
		const createdView: CurriculumVersionView = {
			version: created,
			startAcademicYearName: start.name,
			endAcademicYearName: end?.name ?? null
		};
		versions = [createdView, ...versions];
		await loadVersion(createdView);
	}

	async function cloneVersion(sourceVersionId: string, draft: CloneCurriculumVersionRequest) {
		const options = await requestCreateOptions();
		if (!options) throw new Error('ไม่มีสิทธิ์สร้างรุ่นหลักสูตร');
		const created = await cloneCurriculumVersionDraft(sourceVersionId, draft);
		const start = options.academicYears.find((year) => year.id === created.startAcademicYearId);
		const end = options.academicYears.find((year) => year.id === created.endAcademicYearId);
		if (!start) throw new Error('สร้างรุ่นสำเร็จแต่ไม่พบชื่อปีเริ่มใช้ กรุณาโหลดหน้าใหม่');
		const createdView: CurriculumVersionView = {
			version: created,
			startAcademicYearName: start.name,
			endAcademicYearName: end?.name ?? null
		};
		versions = [createdView, ...versions.filter((view) => view.version.id !== created.id)];
		await loadVersion(createdView);
	}

	async function createProgram(draft: CreateStudyProgramRequest) {
		if (!selectedVersion || !workspace) return;
		const created = await createStudyProgram(selectedVersion.version.id, draft);
		applyWorkspace({
			...workspace,
			programs: [...workspace.programs, created]
		});
	}

	async function saveStructure(
		studyProgramId: string,
		rowVersion: number,
		requirements: CurriculumStructureRequirementInput[]
	) {
		applyWorkspace(await replaceCurriculumStructure(studyProgramId, { rowVersion, requirements }));
	}

	async function saveTermSlots(slots: CurriculumTermSlotInput[]) {
		if (!selectedVersion || !workspace) return;
		applyWorkspace(
			await replaceCurriculumTermSlots(selectedVersion.version.id, {
				rowVersion: workspace.rowVersion,
				slots
			})
		);
	}

	async function openEditor() {
		const options = await requestManagementOptions();
		if (options) editorOpen = true;
	}

	async function publishVersion(id: string, rowVersion: number) {
		const updated = await publishCurriculumVersion(id, { rowVersion });
		versions = versions.map((view) =>
			view.version.id === updated.id ? { ...view, version: updated } : view
		);
		selectedVersion = selectedVersion ? { ...selectedVersion, version: updated } : null;
		applyWorkspace(await getCurriculumStructureWorkspace(id));
	}

	afterNavigate(({ to }) => {
		const targetUrl = to?.url ?? page.url;
		const nextAlignmentContextKey = deliveryContextKey(targetUrl);
		if (nextAlignmentContextKey !== loadedAlignmentContextKey) {
			loadedAlignmentContextKey = nextAlignmentContextKey;
			void loadAlignment(targetUrl);
		}
		const requestedVersionId = to?.url.searchParams.get('versionId') ?? null;
		if (!initialized || versions.length === 0) return;
		const target =
			versions.find((version) => version.version.id === requestedVersionId) ?? versions[0] ?? null;
		if (target && target.version.id !== selectedVersion?.version.id)
			void loadVersion(target, false);
	});

	onMount(() => {
		loadedAlignmentContextKey = deliveryContextKey(page.url);
		void loadDetail();
		void loadAlignment(page.url);
		return () => {
			detailRequest.abort();
			versionRequest.abort();
			alignmentRequest.abort();
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
				canManage={canManageAcademicCurriculum}
				onSelectVersion={loadVersion}
				onRequestCreateOptions={requestCreateOptions}
				onCreateVersion={createVersion}
				onCloneVersion={cloneVersion}
			/>

			{#if deliveryContext}
				{#if alignmentLoading && !alignmentWorkspace}
					<PageSkeleton variant="cards" rows={3} />
				{:else if alignmentError && !alignmentWorkspace}
					<PageState
						variant="error"
						title="โหลดข้อมูลเทียบหลักสูตรไม่สำเร็จ"
						description={alignmentError}
						actionLabel="ลองอีกครั้ง"
						onaction={() => loadAlignment(page.url)}
					/>
				{:else if alignmentWorkspace}
					<CurriculumDeliveryAlignmentPanel
						workspace={alignmentWorkspace}
						{curriculumId}
						studyProgramId={deliveryContext.studyProgramId}
						academicYearId={deliveryContext.academicYearId}
						academicTermId={deliveryContext.academicTermId}
					/>
				{/if}
			{/if}

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
			{:else if selectedVersion && workspace}
				{#key selectedVersion.version.id}
					<div class="space-y-4">
						<CurriculumStructureToolbar
							{workspace}
							bind:viewMode
							bind:gradeLevelId={selectedGradeLevelId}
							bind:studyProgramId={selectedStudyProgramId}
							canManage={canManageAcademicCurriculum}
							onEdit={() => void openEditor()}
						/>

						{#if validationBlockers.length > 0}
							<div class="rounded-xl border border-destructive/30 bg-destructive/5 p-3">
								<h3 class="font-semibold text-destructive">ข้อมูลที่ต้องแก้ก่อนเผยแพร่</h3>
								<ul class="mt-2 space-y-1 text-sm text-muted-foreground">
									{#each validationBlockers as blocker (blocker.key)}
										<li>• {blocker.message}</li>
									{/each}
								</ul>
							</div>
						{/if}

						{#if workspace.programs.length === 0 || workspace.gradeLevels.length === 0}
							<PageState
								title={workspace.programs.length === 0
									? 'ยังไม่มีแผนการเรียน'
									: 'หลักสูตรยังไม่มีระดับชั้น'}
								description={workspace.programs.length === 0
									? 'เปิดตัวจัดโครงสร้างเพื่อเพิ่มแผนการเรียนแรก'
									: 'แก้ระดับชั้นของหลักสูตรก่อนจัดรายวิชา'}
							/>
						{:else if viewMode === 'comparison'}
							<CurriculumProgramComparison {workspace} gradeLevelId={selectedGradeLevelId} />
						{:else}
							<CurriculumTermDocument
								{workspace}
								studyProgramId={selectedStudyProgramId}
								gradeLevelId={selectedGradeLevelId}
							/>
						{/if}

						{#if canManageAcademicCurriculum && selectedVersion.version.status === 'draft'}
							<div class="flex justify-end">
								<Button
									disabled={workspace.validation.blockers.length > 0 ||
										workspace.requirements.length === 0}
									onclick={() =>
										void publishVersion(selectedVersion!.version.id, workspace!.rowVersion)}
								>
									เผยแพร่รุ่นหลักสูตร
								</Button>
							</div>
						{/if}
					</div>
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

{#if editorOpen && workspace && selectedManagementOptions}
	<CurriculumStructureEditor
		{workspace}
		managementOptions={selectedManagementOptions}
		onSaveStructure={saveStructure}
		onSaveTermSlots={saveTermSlots}
		onCreateProgram={createProgram}
		onClose={() => (editorOpen = false)}
	/>
{/if}
