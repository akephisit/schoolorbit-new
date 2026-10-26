<script lang="ts">
	import { tick } from 'svelte';
	import CurriculumQuickActionForm from '#lib/components/academic-core/CurriculumQuickActionForm.svelte';
	import {
		captureCurriculumQuickAction,
		type CurriculumQuickAction,
		type CurriculumQuickActionBaseline
	} from '#lib/academic-core/curriculum-quick-actions.js';
	import CurriculumProgramCreateDialog from '#lib/components/academic-core/CurriculumProgramCreateDialog.svelte';
	import CurriculumDeliveryAlignmentPanel from '#lib/components/academic-core/CurriculumDeliveryAlignmentPanel.svelte';
	import {
		getHomeroomDeliveryWorkspace,
		type HomeroomDeliveryWorkspace
	} from '#lib/api/learning-delivery.js';
	import { untrack } from 'svelte';
	import {
		sameCoveredGrades,
		preserveNewerCreditCriteria
	} from '#lib/academic-core/curriculum-credit-criteria.js';
	import { orderedCurriculumPrograms } from '#lib/academic-core/curriculum-program-order.js';
	import { LatestRequest, isAbortError } from '#lib/async/latest-request.js';
	import {
		getCurriculumStructureWorkspace,
		curriculumViewSearch,
		getCurriculumManagementOptions,
		createStudyProgram,
		replaceCurriculumStructure,
		replaceCurriculumTermSlots,
		replaceStudyProgramOrder,
		replaceCurriculumCreditCriteria,
		type CurriculumCreditCriterion,
		type CurriculumCreditCriteriaResult,
		type CurriculumStructureWorkspace,
		type CurriculumManagementOptions,
		type CurriculumEdition,
		type CreateStudyProgramRequest,
		type CurriculumStructureRequirementInput,
		type CurriculumTermSlotInput,
		type StudyProgramOrderSnapshot
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
	let quickBaseline = $state.raw<CurriculumQuickActionBaseline | null>(null);
	let programWriting = $state(false);
	let quickReturnFocus: HTMLElement | null = null;
	const quickOptionsRequest = new LatestRequest();
	let options = $state.raw<CurriculumManagementOptions | null>(null);
	let editorLoading = $state(false);
	let viewMode = $state<'comparison' | 'document'>('comparison');
	let gradeLevelId = $state('');
	let studyProgramId = $state('');
	let termSlotIds = $state<string[]>([]);
	let termsInitialized = false;
	let mutationRevision = 0;
	const workspaceRequest = new LatestRequest();
	const editorRequest = new LatestRequest();
	const alignmentRequest = new LatestRequest();
	const programOrderReviewRequest = new LatestRequest();
	const creditCriteriaReviewRequest = new LatestRequest();
	function abortEditorReviews() {
		programOrderReviewRequest.abort();
		creditCriteriaReviewRequest.abort();
	}
	let canManage = $derived(
		$can.has(PERMISSIONS.ACADEMIC_CURRICULUM_MANAGE_SCHOOL) && !!workspace?.level.draftId
	);
	$effect(() => {
		if (!canManage) {
			quickOptionsRequest.abort();
			editorRequest.abort();
			quickBaseline = null;
			editorOpen = false;
			editorLoading = false;
		}
	});
	function apply(result: CurriculumStructureWorkspace) {
		const previousTerms = workspace?.termSlots.map((term) => term.id) ?? [];
		if (!termsInitialized) {
			termSlotIds = result.termSlots.map((term) => term.id);
			termsInitialized = true;
		} else {
			const showingAllTerms = previousTerms.every((termId) => termSlotIds.includes(termId));
			termSlotIds = result.termSlots
				.filter((term) => showingAllTerms || termSlotIds.includes(term.id))
				.map((term) => term.id);
		}
		workspace = {
			...result,
			level: preserveNewerCreditCriteria(workspace?.level ?? null, result.level),
			programs: orderedCurriculumPrograms(result.programs)
		};
		if (gradeLevelId !== 'all' && !result.gradeLevels.some((g) => g.id === gradeLevelId))
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
			termsInitialized = false;
			edition = null;
			options = null;
			alignment = null;
			alignmentError = '';
			alignmentLoading = !!ap;
			studyProgramId = data.studyProgramId;
			editorOpen = false;
			quickBaseline = null;
			quickReturnFocus = null;
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
			quickOptionsRequest.abort();
			alignmentRequest.abort();
			abortEditorReviews();
		};
	});
	function closeEditor() {
		abortEditorReviews();
		editorOpen = false;
	}
	async function refresh() {
		abortEditorReviews();
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
		if (!canManage || quickBaseline || editorOpen || programWriting) return;
		abortEditorReviews();
		const levelId = data.levelId;
		const { revision, signal } = editorRequest.begin();
		editorLoading = true;
		error = '';
		try {
			const result = await getCurriculumManagementOptions(levelId, { signal });
			if (
				editorRequest.isCurrent(revision) &&
				levelId === data.levelId &&
				canManage &&
				!quickBaseline
			) {
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
	function openQuickAction(action: CurriculumQuickAction) {
		if (!workspace || !canManage || editorOpen || editorLoading || quickBaseline || programWriting)
			return;
		const baseline = captureCurriculumQuickAction(workspace, action, canManage);
		if (!baseline) return;
		quickReturnFocus =
			document.activeElement instanceof HTMLElement ? document.activeElement : null;
		quickBaseline = baseline;
	}
	function closeQuickAction() {
		const context = quickBaseline?.action.context;
		quickOptionsRequest.abort();
		quickBaseline = null;
		void tick().then(() => {
			if (quickReturnFocus?.isConnected) quickReturnFocus.focus();
			else if (context)
				document
					.querySelector<HTMLElement>(
						`[data-term-header][data-program-id="${context.studyProgramId}"][data-grade-id="${context.gradeLevelId}"][data-term-id="${context.termSlotId}"]`
					)
					?.focus();
			quickReturnFocus = null;
		});
	}
	async function loadQuickOptions() {
		const baseline = quickBaseline;
		if (!baseline || !canManage) throw new DOMException('บริบทเปลี่ยนไปแล้ว', 'AbortError');
		const { revision, signal } = quickOptionsRequest.begin();
		const result = await getCurriculumManagementOptions(baseline.levelId, { signal });
		if (
			!quickOptionsRequest.isCurrent(revision) ||
			quickBaseline !== baseline ||
			workspace?.level.draftId !== baseline.draftId ||
			data.levelId !== baseline.levelId ||
			!canManage
		)
			throw new DOMException('บริบทเปลี่ยนไปแล้ว', 'AbortError');
		return result;
	}
	async function saveQuickAction(requirements: CurriculumStructureRequirementInput[]) {
		const baseline = quickBaseline;
		if (!baseline) throw new DOMException('บริบทเปลี่ยนไปแล้ว', 'AbortError');
		await saveStructure(
			baseline.action.context.studyProgramId,
			baseline.rowVersion,
			requirements,
			baseline
		);
	}
	async function latestQuickContext() {
		const baseline = quickBaseline;
		if (!baseline) return;
		const { revision, signal } = workspaceRequest.begin();
		const initialRevision = mutationRevision;
		const result = await getCurriculumStructureWorkspace(baseline.levelId, {
			signal,
			...data.view
		});
		if (
			!workspaceRequest.isCurrent(revision) ||
			quickBaseline !== baseline ||
			mutationRevision !== initialRevision ||
			data.levelId !== baseline.levelId ||
			result.level.editionId !== data.editionId
		)
			throw new DOMException('บริบทเปลี่ยนไปแล้ว', 'AbortError');
		mutationRevision++;
		apply(result);
		closeQuickAction();
	}
	async function saveStructure(
		programId: string,
		rowVersion: number,
		requirements: CurriculumStructureRequirementInput[],
		baseline?: CurriculumQuickActionBaseline
	) {
		if (!workspace || !canManage || programWriting)
			throw new Error('ไม่สามารถบันทึกโครงสร้างในขณะนี้');
		const selected = workspace.level;
		const structureContext = data.structure;
		const draftId = baseline?.draftId ?? selected.draftId;
		if (
			!draftId ||
			(baseline && (baseline.levelId !== data.levelId || selected.draftId !== draftId))
		)
			throw new DOMException('บริบทเปลี่ยนไปแล้ว', 'AbortError');
		abortEditorReviews();
		programWriting = true;
		try {
			const result = await replaceCurriculumStructure(programId, {
				rowVersion,
				requirements,
				draftId
			});
			if (
				data.structure !== structureContext ||
				data.levelId !== selected.id ||
				workspace?.level.draftId !== draftId ||
				result.level.id !== selected.id ||
				result.level.draftId !== draftId
			)
				throw new DOMException('บริบทเปลี่ยนไปแล้ว', 'AbortError');
			mutationRevision++;
			apply(result);
		} finally {
			programWriting = false;
		}
	}

	async function saveSlots(rowVersion: number, slots: CurriculumTermSlotInput[]) {
		if (!workspace) return;
		const draftId = workspace.level.draftId;
		if (!draftId) return;
		const selected = workspace.level;
		abortEditorReviews();
		const result = await replaceCurriculumTermSlots(workspace.level.id, {
			rowVersion,
			draftId,
			slots
		});
		if (data.levelId !== selected.id || workspace?.level.draftId !== selected.draftId) return;
		mutationRevision++;
		apply(result);
	}
	function patchProgramOrder(result: StudyProgramOrderSnapshot, levelId: string, draftId: string) {
		if (data.levelId !== levelId || workspace?.level.draftId !== draftId) return;
		mutationRevision++;
		workspace = {
			...workspace,
			programs: orderedCurriculumPrograms(result.programs),
			rowVersion: result.rowVersion,
			level: { ...workspace.level, rowVersion: result.rowVersion }
		};
	}
	async function saveProgramOrder(rowVersion: number, studyProgramIds: string[]) {
		if (!workspace?.level.draftId) throw new Error('เปิดร่างแก้ไขก่อนจัดลำดับแผน');
		const { id, draftId } = workspace.level;
		abortEditorReviews();
		const result = await replaceStudyProgramOrder(id, { draftId, rowVersion, studyProgramIds });
		patchProgramOrder(result, id, draftId);
		return result;
	}
	async function reviewProgramOrder() {
		if (!workspace?.level.draftId) throw new Error('เปิดร่างแก้ไขก่อนจัดลำดับแผน');
		const { id, draftId } = workspace.level;
		creditCriteriaReviewRequest.abort();
		const { revision, signal } = programOrderReviewRequest.begin();
		const initialMutationRevision = mutationRevision;
		const latest = await getCurriculumStructureWorkspace(id, { signal, ...data.view });
		if (
			!programOrderReviewRequest.isCurrent(revision) ||
			!editorOpen ||
			data.levelId !== id ||
			workspace?.level.draftId !== draftId ||
			mutationRevision !== initialMutationRevision ||
			latest.level.id !== id ||
			latest.level.draftId !== draftId
		)
			throw new DOMException('ข้อมูลเปลี่ยนไปแล้ว กรุณาโหลดลำดับล่าสุดเพื่อตรวจสอบ', 'AbortError');
		const result: StudyProgramOrderSnapshot = {
			programs: latest.programs,
			rowVersion: latest.level.rowVersion
		};
		patchProgramOrder(result, id, draftId);
		return result;
	}
	function patchCreditCriteria(
		result: CurriculumCreditCriteriaResult,
		levelId: string,
		draftId: string
	) {
		if (
			data.levelId !== levelId ||
			workspace?.level.draftId !== draftId ||
			result.rowVersion < workspace.level.creditCriteriaRowVersion
		)
			return false;
		mutationRevision++;
		workspace = {
			...workspace,
			level: {
				...workspace.level,
				creditCriteria: result.criteria,
				creditCriteriaRowVersion: result.rowVersion
			}
		};
		return true;
	}
	async function saveCreditCriteria(rowVersion: number, criteria: CurriculumCreditCriterion[]) {
		if (!workspace?.level.draftId) throw new Error('เปิดร่างแก้ไขก่อนกำหนดเกณฑ์');
		const { id, draftId } = workspace.level;
		abortEditorReviews();
		const result = await replaceCurriculumCreditCriteria(id, { draftId, rowVersion, criteria });
		if (!patchCreditCriteria(result, id, draftId))
			throw new DOMException('เกณฑ์เปลี่ยนไปแล้ว กรุณาตรวจสอบเกณฑ์ล่าสุด', 'AbortError');
		return result;
	}
	async function reviewCreditCriteria() {
		if (!workspace?.level.draftId) throw new Error('เปิดร่างแก้ไขก่อนกำหนดเกณฑ์');
		const { id, draftId } = workspace.level;
		programOrderReviewRequest.abort();
		const { revision, signal } = creditCriteriaReviewRequest.begin();
		const initialMutationRevision = mutationRevision;
		const latest = await getCurriculumStructureWorkspace(id, { signal, ...data.view });
		if (
			!creditCriteriaReviewRequest.isCurrent(revision) ||
			!editorOpen ||
			data.levelId !== id ||
			workspace?.level.draftId !== draftId ||
			mutationRevision !== initialMutationRevision ||
			latest.level.id !== id ||
			latest.level.draftId !== draftId
		)
			throw new DOMException('ข้อมูลเปลี่ยนไปแล้ว กรุณาโหลดเกณฑ์ล่าสุดเพื่อตรวจสอบ', 'AbortError');
		if (!sameCoveredGrades(latest.level.gradeLevelIds, workspace.level.gradeLevelIds))
			throw new Error(
				'ชั้นที่ครอบคลุมเปลี่ยนไป ร่างเกณฑ์และร่างส่วนอื่นยังอยู่ กรุณาปิดตัวแก้ไขแล้วโหลดหน้าใหม่เพื่อตรวจสอบชั้นก่อนบันทึก'
			);
		const result = {
			criteria: latest.level.creditCriteria,
			rowVersion: latest.level.creditCriteriaRowVersion
		};
		if (!patchCreditCriteria(result, id, draftId))
			throw new DOMException('เกณฑ์เปลี่ยนไปแล้ว กรุณาตรวจสอบเกณฑ์ล่าสุด', 'AbortError');
		return result;
	}
	async function createProgram(request: CreateStudyProgramRequest) {
		if (!workspace) return;
		abortEditorReviews();
		await createStudyProgram(workspace.level.id, request);
		mutationRevision++;
		await refresh();
	}
	function copied() {
		abortEditorReviews();
		mutationRevision++;
		void refresh();
	}
	function programCreated() {
		abortEditorReviews();
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
					bind:termSlotIds
					{canManage}
					onEdit={edit}
					busy={editorLoading || !!quickBaseline || programWriting}
				/>
				{#if canManage}<p class="text-xs text-muted-foreground">
						คลิกขวาที่หัวภาคเรียน หมวด หรือรายการเพื่อจัดการ · ใช้ Shift+F10 หรือปุ่มเมนูบนแป้นพิมพ์
						· อุปกรณ์สัมผัสใช้ปุ่ม ⋯
					</p>{/if}
				{#if viewMode === 'comparison'}<CurriculumProgramComparison
						{canManage}
						onQuickAction={openQuickAction}
						busy={editorOpen || editorLoading || !!quickBaseline || programWriting}
						{workspace}
						{gradeLevelId}
						{termSlotIds}
					/>{:else}<CurriculumTermDocument
						{canManage}
						onQuickAction={openQuickAction}
						busy={editorOpen || editorLoading || !!quickBaseline || programWriting}
						{workspace}
						{gradeLevelId}
						{studyProgramId}
						{termSlotIds}
					/>{/if}
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
		initialProgram={studyProgramId}
		initialGrade={gradeLevelId}
		initialTerm={termSlotIds.length === 1 ? termSlotIds[0] : ''}
		onSaveStructure={saveStructure}
		onSaveTermSlots={saveSlots}
		onCreateProgram={createProgram}
		onSaveProgramOrder={saveProgramOrder}
		onReviewProgramOrder={reviewProgramOrder}
		onSaveCreditCriteria={saveCreditCriteria}
		onReviewCreditCriteria={reviewCreditCriteria}
		onClose={closeEditor}
	/>{/if}

{#if quickBaseline}<CurriculumQuickActionForm
		baseline={quickBaseline}
		onLoadOptions={loadQuickOptions}
		onSave={saveQuickAction}
		onClose={closeQuickAction}
		onLatest={latestQuickContext}
	/>{/if}
