<script lang="ts">
	import CurriculumProgramCreateDialog from '#lib/components/academic-core/CurriculumProgramCreateDialog.svelte';
	import CurriculumDeliveryAlignmentPanel from '#lib/components/academic-core/CurriculumDeliveryAlignmentPanel.svelte';
	import {
		getHomeroomDeliveryWorkspace,
		type HomeroomDeliveryWorkspace
	} from '#lib/api/learning-delivery.js';
	import { untrack } from 'svelte';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import {
		getCurriculumStructureWorkspace,
		curriculumViewSearch,
		getCurriculumManagementOptions,
		createStudyProgram,
		replaceCurriculumStructure,
		replaceCurriculumTermSlots,
		type CurriculumStructureWorkspace,
		type CurriculumManagementOptions,
		type CurriculumEdition,
		type CreateStudyProgramRequest,
		type CurriculumStructureRequirementInput,
		type CurriculumTermSlotInput
	} from '#lib/api/academic-core.js';
	import CurriculumProgramComparison from '#lib/components/academic-core/CurriculumProgramComparison.svelte';
	import CurriculumTermDocument from '#lib/components/academic-core/CurriculumTermDocument.svelte';
	import CurriculumStructureToolbar from '#lib/components/academic-core/CurriculumStructureToolbar.svelte';
	import CurriculumStructureEditor from '#lib/components/academic-core/CurriculumStructureEditor.svelte';
	import CurriculumProgramCopyDialog from '#lib/components/academic-core/CurriculumProgramCopyDialog.svelte';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	let alignment = $state.raw<HomeroomDeliveryWorkspace | null>(null);
	let alignmentError = $state('');
	let alignmentLoading = $state(false);
	let workspace = $state.raw<CurriculumStructureWorkspace | null>(null);
	let edition = $state.raw<CurriculumEdition | null>(null);
	let loading = $state(true);
	let error = $state('');
	let editorOpen = $state(false);
	let options = $state.raw<CurriculumManagementOptions | null>(null);
	let editorLoading = $state(false);
	let viewMode = $state<'comparison' | 'document'>('comparison');
	let gradeLevelId = $state('');
	let studyProgramId = $state('');
	let mutationRevision = 0;
	const workspaceRequest = new LatestRequest();
	const editorRequest = new LatestRequest();
	const alignmentRequest = new LatestRequest();
	let canManage = $derived(
		$can.has(PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_SCHOOL) && !!workspace?.level.draftId
	);
	function apply(result: CurriculumStructureWorkspace) {
		workspace = result;
		if (!result.gradeLevels.some((g) => g.id === gradeLevelId))
			gradeLevelId = result.gradeLevels[0]?.id ?? '';
		if (!result.programs.some((p) => p.id === studyProgramId))
			studyProgramId = result.programs[0]?.id ?? '';
	}
	$effect.pre(() => {
		const pending = data.structure;
		const ep = data.edition;
		const ap = data.alignment;
		const revision = mutationRevision;
		let current = true;
		untrack(() => {
			workspace = null;
			edition = null;
			options = null;
			alignment = null;
			alignmentError = '';
			alignmentLoading = !!ap;
			studyProgramId = data.studyProgramId;
			editorOpen = false;
			editorLoading = false;
			loading = true;
			error = '';
		});
		void pending.then((r) => {
			if (!current) return;
			if (r.ok) {
				if (revision === mutationRevision) apply(r.data);
			} else error = r.error;
			loading = false;
		});
		void ep.then((r) => {
			if (current && r.ok) edition = r.data;
		});
		if (ap)
			void ap.then((r) => {
				if (!current) return;
				if (r.ok) alignment = r.data;
				else alignmentError = r.error;
				alignmentLoading = false;
			});
		return () => {
			current = false;
			workspaceRequest.abort();
			editorRequest.abort();
			alignmentRequest.abort();
		};
	});
	async function refresh() {
		const levelId = data.levelId;
		const editionId = data.editionId;
		const { revision, signal } = workspaceRequest.begin();
		const initialMutationRevision = mutationRevision;
		loading = true;
		error = '';
		try {
			const result = await getCurriculumStructureWorkspace(levelId, { signal, ...data.view });
			if (
				!workspaceRequest.isCurrent(revision) ||
				data.levelId !== levelId ||
				mutationRevision !== initialMutationRevision
			)
				return;
			if (result.level.editionId !== editionId)
				throw new Error('ระดับการศึกษาไม่อยู่ในฉบับที่เลือก');
			apply(result);
		} catch (e) {
			if (workspaceRequest.isCurrent(revision) && !isAbortError(e))
				error = e instanceof Error ? e.message : 'โหลดแผนการเรียนไม่สำเร็จ';
		} finally {
			if (workspaceRequest.isCurrent(revision)) loading = false;
		}
	}
	async function edit() {
		if (!canManage) return;
		const levelId = data.levelId;
		const { revision, signal } = editorRequest.begin();
		editorLoading = true;
		error = '';
		try {
			const result = await getCurriculumManagementOptions(levelId, { signal });
			if (editorRequest.isCurrent(revision) && levelId === data.levelId) {
				options = result;
				editorOpen = true;
			}
		} catch (e) {
			if (editorRequest.isCurrent(revision) && !isAbortError(e))
				error = e instanceof Error ? e.message : 'โหลดตัวเลือกจัดการไม่สำเร็จ';
		} finally {
			if (editorRequest.isCurrent(revision)) editorLoading = false;
		}
	}
	async function saveStructure(
		programId: string,
		rowVersion: number,
		requirements: CurriculumStructureRequirementInput[]
	) {
		if (!workspace) return;
		const draftId = workspace.level.draftId;
		if (!draftId) return;
		const selected = workspace.level;
		const result = await replaceCurriculumStructure(programId, {
			rowVersion,
			requirements,
			draftId
		});
		if (data.levelId !== selected.id || workspace?.level.draftId !== selected.draftId) return;
		mutationRevision++;
		apply(result);
	}
	async function saveSlots(slots: CurriculumTermSlotInput[]) {
		if (!workspace) return;
		const draftId = workspace.level.draftId;
		if (!draftId) return;
		const selected = workspace.level;
		const result = await replaceCurriculumTermSlots(workspace.level.id, {
			rowVersion: workspace.level.rowVersion,
			draftId,
			slots
		});
		if (data.levelId !== selected.id || workspace?.level.draftId !== selected.draftId) return;
		mutationRevision++;
		apply(result);
	}
	async function createProgram(request: CreateStudyProgramRequest) {
		if (!workspace) return;
		await createStudyProgram(workspace.level.id, request);
		mutationRevision++;
		await refresh();
	}
	function copied() {
		mutationRevision++;
		void refresh();
	}
	function programCreated() {
		mutationRevision++;
		void refresh();
	}
	async function retryAlignment() {
		const context = data.alignmentContext;
		if (!context) return;
		const { revision, signal } = alignmentRequest.begin();
		alignmentLoading = true;
		alignmentError = '';
		try {
			const result = await getHomeroomDeliveryWorkspace(
				context.academicYearId,
				context.academicTermId,
				{ deliveryVersionId: context.deliveryVersionId, signal }
			);
			if (alignmentRequest.isCurrent(revision)) alignment = result;
		} catch (e) {
			if (alignmentRequest.isCurrent(revision) && !isAbortError(e))
				alignmentError = e instanceof Error ? e.message : 'โหลดข้อมูลเทียบการเปิดสอนไม่สำเร็จ';
		} finally {
			if (alignmentRequest.isCurrent(revision)) alignmentLoading = false;
		}
	}
</script>

<PageShell
	title={workspace?.level.nameTh ?? 'แผนการเรียน'}
	description={`${edition?.name ?? 'ฉบับหลักสูตร'} → ระดับการศึกษา → แผนการเรียน → ชั้น → ภาคเรียน`}
>
	{#snippet actions()}<Button
			variant="outline"
			href={`/staff/academic/curricula/${data.editionId}${curriculumViewSearch(data.view)}`}
			>กลับฉบับหลักสูตร</Button
		>{/snippet}
	<div class="space-y-5">
		{#if data.alignmentContext}{#if alignmentLoading && !alignment}<PageSkeleton
					variant="cards"
					rows={2}
				/>{:else if alignmentError && !alignment}<PageState
					variant="error"
					title="โหลดข้อมูลเทียบการเปิดสอนไม่สำเร็จ"
					description={alignmentError}
					actionLabel="ลองอีกครั้ง"
					onaction={retryAlignment}
				/>{:else if alignment}<CurriculumDeliveryAlignmentPanel
					workspace={alignment}
					curriculumLevelId={data.levelId}
					studyProgramId={data.alignmentContext.studyProgramId ?? undefined}
					academicYearId={data.alignmentContext.academicYearId}
					academicTermId={data.alignmentContext.academicTermId}
				/>{/if}{/if}

		{#if workspace?.level.draftId}<p class="rounded-xl border bg-muted p-3 text-sm">
				กำลังแก้ไขร่าง · บันทึกที่นี่แล้วกลับไปเผยแพร่ทั้งฉบับ
				การเปิดสอนและตารางเดิมยังใช้ข้อมูลเดิม
			</p>
		{:else if data.view.publicationId}<p class="rounded-xl border bg-muted p-3 text-sm">
				ประวัติการเผยแพร่ · อ่านอย่างเดียว
			</p>{/if}
		{#if loading && !workspace}<PageSkeleton
				variant="cards"
				rows={3}
			/>{:else if error && !workspace}<PageState
				variant="error"
				title="โหลดแผนการเรียนไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={refresh}
			/>{/if}
		{#if workspace}<section
				class="space-y-3 rounded-2xl border bg-card p-4"
				data-testid="curriculum-level-ready"
			>
				<div class="flex flex-wrap items-center justify-between gap-3">
					<div>
						<p class="text-sm text-muted-foreground">{workspace.level.editionName}</p>
						<h2 class="font-semibold">{workspace.level.nameTh}</h2>
					</div>
					{#if canManage}<div class="flex flex-wrap gap-2">
							<CurriculumProgramCopyDialog
								level={workspace.level}
								onCopied={copied}
							/><CurriculumProgramCreateDialog
								levelId={workspace.level.id}
								draftId={workspace.level.draftId ?? ''}
								isFirst={workspace.programs.length === 0}
								onCreated={programCreated}
							/>
						</div>{/if}
				</div>
				<p class="text-sm text-muted-foreground">
					ครอบคลุม {workspace.gradeLevels.map((g) => g.short_name ?? g.name).join(' · ')} · {workspace
						.programs.length} แผนการเรียน
				</p>
			</section>
			{#if error && workspace}<div role="alert" class="flex gap-3 text-sm text-destructive">
					<span>{error}</span><Button variant="outline" onclick={refresh}>ลองอีกครั้ง</Button>
				</div>{/if}
			{#if workspace.programs.length}<CurriculumStructureToolbar
					{workspace}
					bind:viewMode
					bind:gradeLevelId
					bind:studyProgramId
					{canManage}
					onEdit={edit}
					busy={editorLoading}
				/>{#if viewMode === 'comparison'}<CurriculumProgramComparison
						{workspace}
						{gradeLevelId}
					/>{:else}<CurriculumTermDocument {workspace} {gradeLevelId} {studyProgramId} />{/if}
			{:else}<PageState
					variant="empty"
					title="ยังไม่มีแผนการเรียน"
					description="เพิ่มแผนการเรียนหรือคัดลอกแผนจากฉบับที่เผยแพร่แล้ว แล้วกำหนดรายวิชาและกิจกรรมตามชั้นกับภาคเรียน"
				/>{/if}
		{/if}
	</div>
</PageShell>
{#if editorOpen && workspace && options}<CurriculumStructureEditor
		{workspace}
		managementOptions={options}
		onSaveStructure={saveStructure}
		onSaveTermSlots={saveSlots}
		onCreateProgram={createProgram}
		onClose={() => (editorOpen = false)}
	/>{/if}
