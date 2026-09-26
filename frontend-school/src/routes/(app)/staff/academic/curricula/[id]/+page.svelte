<script lang="ts">
	import { pushState, replaceState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { untrack } from 'svelte';
	import { SvelteMap, SvelteURLSearchParams } from 'svelte/reactivity';
	import { buildCurriculumValidationNoticeViews } from '$lib/academic/curriculum-structure';
	import {
		curriculumAlignmentContextKey,
		readCurriculumAlignmentContext
	} from '$lib/academic-core/curriculum-detail-route';
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
	import { PageSkeleton, PageState, RegionUpdatingState } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { ArrowLeft } from '@lucide/svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const curriculumRequest = new LatestRequest();
	const versionsRequest = new LatestRequest();
	const versionRequest = new LatestRequest();
	const alignmentRequest = new LatestRequest();
	const managementCache = new SvelteMap<string, CurriculumManagementOptions>();

	let curriculum = $state.raw<Curriculum | null>(null);
	let versions = $state.raw<CurriculumVersionView[]>([]);
	let selectedVersion = $state.raw<CurriculumVersionView | null>(null);
	let workspace = $state.raw<CurriculumStructureWorkspace | null>(null);
	let createOptions = $state.raw<CurriculumCreateOptions | null>(null);
	let alignmentWorkspace = $state.raw<HomeroomDeliveryWorkspace | null>(null);
	let curriculumLoading = $state(true);
	let versionsLoading = $state(true);
	let workspaceLoading = $state(false);
	let alignmentLoading = $state(false);
	let curriculumError = $state('');
	let versionsError = $state('');
	let workspaceError = $state('');
	let alignmentError = $state('');
	let editorOpen = $state(false);
	let viewMode = $state<'comparison' | 'document'>('comparison');
	let selectedGradeLevelId = $state('');
	let selectedStudyProgramId = $state('');
	let activeCurriculumId = '';
	let activeStructureKey = '';
	let activeAlignmentKey = '';
	let mutationRevision = 0;
	let curriculumId = $derived(data.curriculumId);
	let deliveryContext = $derived(data.alignmentContext);
	let canManageAcademicCurriculum = $derived(
		$can.hasAny(
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_SCHOOL,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT
		)
	);
	let selectedManagementOptions = $derived(
		managementCache.get(workspace?.curriculumVersion.id ?? selectedVersion?.version.id ?? '') ??
			null
	);
	let validationBlockers = $derived(
		buildCurriculumValidationNoticeViews(workspace?.validation.blockers ?? [])
	);

	function curriculumVersionUrl(
		versionId: string,
		currentUrl: URL = page.url
	): `/staff/academic/curricula/${string}?${string}` {
		const query = new SvelteURLSearchParams(currentUrl.searchParams);
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

	async function retryCurriculum() {
		const { revision, signal } = curriculumRequest.begin();
		curriculumLoading = true;
		curriculumError = '';
		try {
			const loaded = await getCurriculum(curriculumId, { signal });
			if (curriculumRequest.isCurrent(revision)) curriculum = loaded;
		} catch (error) {
			if (isAbortError(error)) return;
			if (curriculumRequest.isCurrent(revision)) {
				curriculumError =
					error instanceof Error ? error.message : 'โหลดรายละเอียดหลักสูตรไม่สำเร็จ';
			}
		} finally {
			if (curriculumRequest.isCurrent(revision)) curriculumLoading = false;
		}
	}

	async function retryVersions() {
		const { revision, signal } = versionsRequest.begin();
		versionsLoading = true;
		versionsError = '';
		try {
			const loaded = await listCurriculumVersions(curriculumId, { signal });
			if (!versionsRequest.isCurrent(revision)) return;
			versions = loaded;
			const requested = page.url.searchParams.get('versionId');
			const selected = loaded.find((view) => view.version.id === requested) ?? loaded[0] ?? null;
			selectedVersion = selected;
			if (selected && selected.version.id !== workspace?.curriculumVersion.id)
				await loadVersion(selected, false);
		} catch (error) {
			if (isAbortError(error)) return;
			if (versionsRequest.isCurrent(revision))
				versionsError = error instanceof Error ? error.message : 'โหลดรายการรุ่นหลักสูตรไม่สำเร็จ';
		} finally {
			if (versionsRequest.isCurrent(revision)) versionsLoading = false;
		}
	}

	async function retryStructure(versionId: string) {
		const { revision, signal } = versionRequest.begin();
		workspaceLoading = true;
		workspaceError = '';
		try {
			const loadedWorkspace = await getCurriculumStructureWorkspace(versionId, { signal });
			if (loadedWorkspace.curriculumVersion.curriculumId !== curriculumId)
				throw new Error('รุ่นหลักสูตรไม่อยู่ในหลักสูตรที่เลือก');
			if (!versionRequest.isCurrent(revision)) return;
			applyWorkspace(loadedWorkspace);
		} catch (error) {
			if (isAbortError(error)) return;
			if (versionRequest.isCurrent(revision)) {
				workspaceError = error instanceof Error ? error.message : 'โหลดรุ่นหลักสูตรไม่สำเร็จ';
			}
		} finally {
			if (versionRequest.isCurrent(revision)) workspaceLoading = false;
		}
	}

	async function loadVersion(version: CurriculumVersionView, updateUrl = true) {
		activeStructureKey = `${curriculumId}:${version.version.id}`;
		selectedVersion = version;
		if (workspace?.curriculumVersion.id !== version.version.id) applyWorkspace(null);
		if (updateUrl) pushState(resolve(curriculumVersionUrl(version.version.id)), page.state);
		await retryStructure(version.version.id);
	}

	async function loadAlignment(url: URL = page.url) {
		const context = readCurriculumAlignmentContext(url);
		if (!context) {
			alignmentRequest.abort();
			alignmentWorkspace = null;
			alignmentError = '';
			alignmentLoading = false;
			return;
		}
		const { revision, signal } = alignmentRequest.begin();
		if (
			alignmentWorkspace &&
			(alignmentWorkspace.academicYearId !== context.academicYearId ||
				alignmentWorkspace.academicTermId !== context.academicTermId)
		)
			alignmentWorkspace = null;
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
		if (!canManageAcademicCurriculum) return null;
		const versionId = workspace?.curriculumVersion.id ?? selectedVersion?.version.id;
		if (!versionId) return null;
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
		mutationRevision += 1;
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
		mutationRevision += 1;
		versions = [createdView, ...versions.filter((view) => view.version.id !== created.id)];
		await loadVersion(createdView);
	}

	async function createProgram(draft: CreateStudyProgramRequest) {
		if (!workspace) return;
		const created = await createStudyProgram(workspace.curriculumVersion.id, draft);
		mutationRevision += 1;
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
		mutationRevision += 1;
		applyWorkspace(await replaceCurriculumStructure(studyProgramId, { rowVersion, requirements }));
	}

	async function saveTermSlots(slots: CurriculumTermSlotInput[]) {
		if (!workspace) return;
		mutationRevision += 1;
		applyWorkspace(
			await replaceCurriculumTermSlots(workspace.curriculumVersion.id, {
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
		mutationRevision += 1;
		versions = versions.map((view) =>
			view.version.id === updated.id ? { ...view, version: updated } : view
		);
		selectedVersion = selectedVersion ? { ...selectedVersion, version: updated } : null;
		applyWorkspace(await getCurriculumStructureWorkspace(id));
	}

	$effect.pre(() => {
		const routeCurriculum = data.curriculum;
		const routeVersions = data.versions;
		const routeStructure = data.structure;
		const routeAlignment = data.alignment;
		const id = data.curriculumId;
		const requestedVersionId = data.requestedVersionId;
		const structureKey = `${id}:${requestedVersionId ?? 'default'}`;
		const alignmentKey = curriculumAlignmentContextKey(data.alignmentContext);
		const initialMutationRevision = mutationRevision;
		let current = true;
		curriculumRequest.abort();
		versionsRequest.abort();
		versionRequest.abort();
		alignmentRequest.abort();
		untrack(() => {
			if (activeCurriculumId !== id) {
				curriculum = null;
				versions = [];
				selectedVersion = null;
				applyWorkspace(null);
				createOptions = null;
				managementCache.clear();
				activeCurriculumId = id;
			}
			if (activeStructureKey !== structureKey) {
				applyWorkspace(null);
				selectedVersion = null;
				activeStructureKey = structureKey;
			}
			if (activeAlignmentKey !== alignmentKey) {
				alignmentWorkspace = null;
				activeAlignmentKey = alignmentKey;
			}
			curriculumLoading = true;
			versionsLoading = true;
			workspaceLoading = true;
			alignmentLoading = Boolean(routeAlignment);
			curriculumError = '';
			versionsError = '';
			workspaceError = '';
			alignmentError = '';
		});
		void routeCurriculum.then((result) => {
			if (!current) return;
			untrack(() => {
				if (result.ok) curriculum = result.data;
				else curriculumError = result.error;
				curriculumLoading = false;
			});
		});
		void routeVersions.then((result) => {
			if (!current) return;
			untrack(() => {
				if (result.ok) {
					if (mutationRevision === initialMutationRevision) versions = result.data;
					else {
						const localIds = new Set(versions.map((view) => view.version.id));
						versions = [
							...versions,
							...result.data.filter((view) => !localIds.has(view.version.id))
						];
					}
					const currentVersionId = page.url.searchParams.get('versionId');
					const selected =
						versions.find((view) => view.version.id === currentVersionId) ?? versions[0] ?? null;
					selectedVersion = selected;
					if (selected) {
						activeStructureKey = `${id}:${selected.version.id}`;
						if (selected.version.id !== currentVersionId) {
							replaceState(resolve(curriculumVersionUrl(selected.version.id)), page.state);
							if (requestedVersionId) void loadVersion(selected, false);
						}
					}
				} else versionsError = result.error;
				versionsLoading = false;
			});
		});
		void routeStructure.then((result) => {
			if (!current) return;
			untrack(() => {
				const routeVersionId = result.ok ? result.data?.curriculumVersion.id : requestedVersionId;
				if (selectedVersion && routeVersionId && routeVersionId !== selectedVersion.version.id)
					return;
				if (result.ok && mutationRevision === initialMutationRevision) applyWorkspace(result.data);
				else if (!result.ok) workspaceError = result.error;
				workspaceLoading = false;
			});
		});
		if (routeAlignment) {
			void routeAlignment.then((result) => {
				if (!current) return;
				untrack(() => {
					if (result.ok) alignmentWorkspace = result.data;
					else alignmentError = result.error;
					alignmentLoading = false;
				});
			});
		}
		return () => {
			current = false;
			curriculumRequest.abort();
			versionsRequest.abort();
			versionRequest.abort();
			alignmentRequest.abort();
		};
	});

	$effect.pre(() => {
		const requested = page.url.searchParams.get('versionId');
		const target = untrack(() => versions.find((view) => view.version.id === requested));
		if (!target || target.version.id === untrack(() => selectedVersion?.version.id)) return;
		void loadVersion(target, false);
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

	<div class="space-y-5">
		{#if curriculumLoading && !curriculum}
			<PageSkeleton variant="cards" rows={2} />
		{:else if curriculumError && !curriculum}
			<PageState
				variant="error"
				title="โหลดรายละเอียดหลักสูตรไม่สำเร็จ"
				description={curriculumError}
				actionLabel="ลองอีกครั้ง"
				onaction={retryCurriculum}
			/>
		{/if}
		{#if curriculum}
			<div
				class="relative space-y-5"
				aria-busy={curriculumLoading}
				data-testid="curriculum-detail-ready"
			>
				{#if curriculumLoading}<RegionUpdatingState label="กำลังอัปเดตรายละเอียดหลักสูตร" />{/if}
				{#if curriculumError && curriculum}
					<div role="alert" class="flex items-center gap-2 text-sm text-destructive">
						<span>{curriculumError}</span>
						<Button size="sm" variant="outline" onclick={retryCurriculum}>ลองอีกครั้ง</Button>
					</div>
				{/if}
				{#if versionsLoading && versions.length === 0}
					<PageSkeleton variant="cards" rows={2} />
				{:else if versionsError && versions.length === 0}
					<PageState
						variant="error"
						title="โหลดรายการรุ่นหลักสูตรไม่สำเร็จ"
						description={versionsError}
						actionLabel="ลองอีกครั้ง"
						onaction={retryVersions}
					/>
				{:else}
					<div class="relative" aria-busy={versionsLoading}>
						{#if versionsLoading}<RegionUpdatingState label="กำลังอัปเดตรายการรุ่นหลักสูตร" />{/if}
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
						{#if versionsError && versions.length}
							<div role="alert" class="mt-2 flex items-center gap-2 text-sm text-destructive">
								<span>{versionsError}</span>
								<Button size="sm" variant="outline" onclick={retryVersions}>ลองอีกครั้ง</Button>
							</div>
						{/if}
					</div>
				{/if}
			</div>
		{/if}

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
				<div class="relative" aria-busy={alignmentLoading}>
					{#if alignmentLoading}<RegionUpdatingState
							label="กำลังอัปเดตข้อมูลเทียบการเปิดสอน"
						/>{/if}
					<CurriculumDeliveryAlignmentPanel
						workspace={alignmentWorkspace}
						{curriculumId}
						studyProgramId={deliveryContext.studyProgramId}
						academicYearId={deliveryContext.academicYearId}
						academicTermId={deliveryContext.academicTermId}
					/>
					{#if alignmentError && alignmentWorkspace}
						<div role="alert" class="mt-2 flex items-center gap-2 text-sm text-destructive">
							<span>{alignmentError}</span>
							<Button size="sm" variant="outline" onclick={() => loadAlignment(page.url)}
								>ลองอีกครั้ง</Button
							>
						</div>
					{/if}
				</div>
			{/if}
		{/if}

		{#if workspaceLoading && !workspace}
			<PageSkeleton variant="cards" rows={4} />
		{:else if workspaceError && !workspace}
			<PageState
				variant="error"
				title="โหลดแผนการเรียนไม่สำเร็จ"
				description={workspaceError}
				actionLabel="ลองอีกครั้ง"
				onaction={() => {
					const versionId = selectedVersion?.version.id ?? data.requestedVersionId;
					if (versionId) void retryStructure(versionId);
				}}
			/>
		{:else if workspace}
			{#key workspace.curriculumVersion.id}
				<div class="relative space-y-4" aria-busy={workspaceLoading}>
					{#if workspaceLoading}<RegionUpdatingState label="กำลังอัปเดตแผนการเรียน" />{/if}
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

					{#if canManageAcademicCurriculum && workspace.curriculumVersion.status === 'draft'}
						<div class="flex justify-end">
							<Button
								disabled={workspace.validation.blockers.length > 0 ||
									workspace.requirements.length === 0}
								onclick={() =>
									void publishVersion(workspace!.curriculumVersion.id, workspace!.rowVersion)}
							>
								เผยแพร่รุ่นหลักสูตร
							</Button>
						</div>
					{/if}
					{#if workspaceError && workspace}
						<div role="alert" class="flex items-center gap-2 text-sm text-destructive">
							<span>{workspaceError}</span>
							<Button
								size="sm"
								variant="outline"
								onclick={() => void retryStructure(workspace!.curriculumVersion.id)}
								>ลองอีกครั้ง</Button
							>
						</div>
					{/if}
				</div>
			{/key}
		{:else if !versionsLoading && !versionsError}
			<PageState
				title="ยังไม่มีรุ่นหลักสูตร"
				description="สร้างรุ่นแบบร่างเพื่อเริ่มกำหนดแผนการเรียนและรายการในแผน"
			/>
		{/if}
	</div>
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
