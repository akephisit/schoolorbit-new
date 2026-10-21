<script lang="ts">
	import type {
		CreateStudyProgramRequest,
		CurriculumCreditCriterion,
		CurriculumCreditCriteriaResult,
		CurriculumManagementOptions,
		CurriculumStructureRequirementInput,
		CurriculumStructureWorkspace,
		CurriculumTermSlotInput,
		StudyProgramOrderResult,
		StudyProgramOrderSnapshot
	} from '#lib/api/academic-core.js';
	import {
		groupStagedRequirements,
		moveStagedRequirementWithinSection,
		appendSelectedCatalogRequirements,
		hasExistingCourseRequirement,
		selectableCatalogOption,
		type StagedCurriculumRequirement
	} from '#lib/academic-core/curriculum-catalog-selection.js';
	import CurriculumCreditCriteriaSettings from './CurriculumCreditCriteriaSettings.svelte';
	import {
		sameCoveredGrades,
		validateCreditCriteria
	} from '#lib/academic-core/curriculum-credit-criteria.js';
	import { curriculumMetric } from '#lib/academic/curriculum-structure.js';
	import { formatEffectiveRange } from '#lib/academic-core/catalog-presentation.js';
	import CurriculumOutdatedBadge from '#lib/components/academic-core/CurriculumOutdatedBadge.svelte';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import CurriculumTermSlotEditor from '#lib/components/academic-core/CurriculumTermSlotEditor.svelte';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import {
		orderedCurriculumPrograms,
		moveCurriculumItem,
		rebaseAfterOwnProgramOrder
	} from '#lib/academic-core/curriculum-program-order.js';
	import { ApiClientError } from '#lib/api/client.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Sheet from '#lib/components/ui/sheet/index.js';
	import * as Table from '#lib/components/ui/table/index.js';
	import {
		ArrowDownToLine,
		ArrowUp,
		ArrowDown,
		Settings,
		RotateCcw,
		Save,
		Search,
		Trash2
	} from '@lucide/svelte';

	let {
		workspace,
		managementOptions,
		initialProgram = '',
		initialGrade = '',
		initialTerm = '',
		onSaveStructure,
		onSaveTermSlots,
		onCreateProgram,
		onSaveProgramOrder,
		onReviewProgramOrder,
		onSaveCreditCriteria,
		onReviewCreditCriteria,
		onClose
	}: {
		workspace: CurriculumStructureWorkspace;
		managementOptions: CurriculumManagementOptions;
		initialProgram?: string;
		initialGrade?: string;
		initialTerm?: string;
		onSaveStructure: (
			studyProgramId: string,
			rowVersion: number,
			requirements: CurriculumStructureRequirementInput[]
		) => Promise<void>;
		onSaveTermSlots: (rowVersion: number, slots: CurriculumTermSlotInput[]) => Promise<void>;
		onCreateProgram: (request: CreateStudyProgramRequest) => Promise<void>;
		onSaveProgramOrder: (
			rowVersion: number,
			studyProgramIds: string[]
		) => Promise<StudyProgramOrderResult>;
		onReviewProgramOrder: () => Promise<StudyProgramOrderSnapshot>;
		onSaveCreditCriteria: (
			rowVersion: number,
			criteria: CurriculumCreditCriterion[]
		) => Promise<CurriculumCreditCriteriaResult>;
		onReviewCreditCriteria: () => Promise<CurriculumCreditCriteriaResult>;
		onClose: () => void;
	} = $props();
	type StagedRequirement = StagedCurriculumRequirement;
	function initialProgramId() {
		return (
			workspace.programs.find((program) => program.id === initialProgram)?.id ??
			workspace.programs[0]?.id ??
			''
		);
	}
	function initialGradeId() {
		return (
			workspace.gradeLevels.find((grade) => grade.id === initialGrade)?.id ??
			workspace.gradeLevels[0]?.id ??
			''
		);
	}
	function initialTermSlotId() {
		return (
			workspace.termSlots.find((term) => term.id === initialTerm)?.id ??
			[...workspace.termSlots].sort((a, b) => a.sequence - b.sequence)[0]?.id ??
			''
		);
	}
	function initialRequirements(): StagedRequirement[] {
		return workspace.requirements.map((requirement) => ({
			studyProgramId: requirement.studyProgramId,
			resourceKind: requirement.resourceKind,
			catalogVersionId: requirement.catalogVersionId,
			gradeLevelId: requirement.gradeLevel.id,
			termSlotId: requirement.termSlotId,
			requirementKind: requirement.requirementKind,
			displayOrder: requirement.displayOrder
		}));
	}
	function initialSlots(): CurriculumTermSlotInput[] {
		return workspace.termSlots.map((slot) => ({
			id: slot.id,
			sequence: slot.sequence,
			termType: slot.termType,
			typeOccurrence: slot.typeOccurrence,
			name: slot.name
		}));
	}

	function initialOrder() {
		return orderedCurriculumPrograms(workspace.programs).map((program) => program.id);
	}
	function initialLevelRevision() {
		return workspace.level.rowVersion;
	}
	function initialCreditCriteria() {
		return structuredClone(workspace.level.creditCriteria);
	}
	function initialCreditCriteriaRevision() {
		return workspace.level.creditCriteriaRowVersion;
	}
	let criteriaDraft = $state.raw<CurriculumCreditCriterion[]>(initialCreditCriteria());
	let criteriaBaseline = $state.raw<CurriculumCreditCriterion[]>(initialCreditCriteria());
	let criteriaRowVersion = $state(initialCreditCriteriaRevision());
	let criteriaSaving = $state(false);
	let criteriaError = $state('');
	let criteriaConflict = $state(false);
	let criteriaDirty = $derived(
		JSON.stringify(criteriaDraft) !== JSON.stringify(criteriaBaseline) ||
			criteriaDraft.some(
				(criterion) => !sameCoveredGrades(criterion.gradeLevelIds, workspace.level.gradeLevelIds)
			)
	);
	async function saveCreditCriteria() {
		if (criteriaSaving || criteriaConflict || !criteriaDirty) return;
		criteriaError = validateCreditCriteria(criteriaDraft);
		if (criteriaError) return;
		criteriaSaving = true;
		try {
			const result = await onSaveCreditCriteria(
				criteriaRowVersion,
				criteriaDraft.map((criterion) => ({
					...criterion,
					targetCredits: curriculumMetric(criterion.targetCredits),
					gradeLevelIds: [...workspace.level.gradeLevelIds]
				}))
			);
			criteriaDraft = structuredClone(result.criteria);
			criteriaBaseline = structuredClone(result.criteria);
			criteriaRowVersion = result.rowVersion;
		} catch (error) {
			criteriaConflict = error instanceof ApiClientError && error.status === 409;
			criteriaError = error instanceof Error ? error.message : 'บันทึกเกณฑ์ไม่สำเร็จ';
		} finally {
			criteriaSaving = false;
		}
	}
	async function reviewCreditCriteria() {
		if (criteriaSaving) return;
		criteriaSaving = true;
		try {
			const result = await onReviewCreditCriteria();
			criteriaBaseline = structuredClone(result.criteria);
			criteriaRowVersion = result.rowVersion;
			criteriaConflict = false;
			criteriaError = '';
		} catch (error) {
			criteriaError = error instanceof Error ? error.message : 'โหลดเกณฑ์ไม่สำเร็จ';
		} finally {
			criteriaSaving = false;
		}
	}
	let settingsOpen = $state(false);
	let orderIds = $state<string[]>(initialOrder());
	let orderRowVersion = $state(initialLevelRevision());
	let slotRowVersion = $state(initialLevelRevision());
	function initialRequirementVersions() {
		return Object.fromEntries(
			workspace.programs.map((program) => [program.id, program.rowVersion])
		);
	}
	let requirementRowVersions = $state.raw<Record<string, number>>(initialRequirementVersions());
	let orderSaving = $state(false);
	let orderError = $state('');
	let orderConflict = $state(false);
	let orderedPrograms = $derived(orderedCurriculumPrograms(workspace.programs));
	let orderDirty = $derived(
		JSON.stringify(orderIds) !== JSON.stringify(orderedPrograms.map((program) => program.id))
	);
	let orderComplete = $derived(
		orderIds.length === orderedPrograms.length &&
			new Set(orderIds).size === orderIds.length &&
			orderedPrograms.every((program) => orderIds.includes(program.id))
	);
	function resetOrder() {
		orderIds = orderedPrograms.map((program) => program.id);
	}
	function moveProgram(index: number, direction: -1 | 1) {
		orderIds = moveCurriculumItem(orderIds, index, index + direction);
	}
	async function saveProgramOrder() {
		if (orderSaving || !orderDirty || !orderComplete || orderConflict) return;
		orderSaving = true;
		orderError = '';
		try {
			const expectedLevelVersion = orderRowVersion;
			const result = await onSaveProgramOrder(expectedLevelVersion, [...orderIds]);
			const baselines = rebaseAfterOwnProgramOrder(
				{ requirementVersions: requirementRowVersions, slotVersion: slotRowVersion },
				result.previousProgramRowVersions,
				result.programs,
				expectedLevelVersion,
				result.rowVersion
			);
			requirementRowVersions = baselines.requirementVersions;
			slotRowVersion = baselines.slotVersion;
			orderIds = orderedCurriculumPrograms(result.programs).map((program) => program.id);
			orderRowVersion = result.rowVersion;
		} catch (error) {
			orderConflict = error instanceof ApiClientError && error.status === 409;
			orderError = error instanceof Error ? error.message : 'บันทึกลำดับแผนไม่สำเร็จ';
		} finally {
			orderSaving = false;
		}
	}
	async function reviewProgramOrder() {
		if (orderSaving) return;
		orderSaving = true;
		try {
			const result = await onReviewProgramOrder();
			orderRowVersion = result.rowVersion;
			orderConflict = false;
			orderError = '';
		} catch (error) {
			orderError = error instanceof Error ? error.message : 'โหลดลำดับล่าสุดไม่สำเร็จ';
		} finally {
			orderSaving = false;
		}
	}
	let open = $state(true);
	let saving = $state(false);
	let errorMessage = $state('');
	let selectedProgramId = $state(initialProgramId());
	let selectedGradeId = $state(initialGradeId());
	let selectedTermSlotId = $state(initialTermSlotId());
	let requirementKind = $state<'required' | 'elective' | 'optional'>('required');
	const requirementKinds: CurriculumStructureRequirementInput['requirementKind'][] = [
		'required',
		'elective',
		'optional'
	];
	function isRequirementKind(
		value: string
	): value is CurriculumStructureRequirementInput['requirementKind'] {
		return requirementKinds.some((kind) => kind === value);
	}
	let resourceKind = $state<'all' | 'basic_course' | 'additional_course' | 'student_development'>(
		'all'
	);
	let search = $state('');
	let selectedCatalogIds = $state.raw<string[]>([]);
	let programDraft = $state({ nameTh: '' });
	let history = $state.raw<StagedRequirement[][]>([]);
	let stagedRequirements = $state.raw<StagedRequirement[]>(initialRequirements());
	let stagedSlots = $state.raw<CurriculumTermSlotInput[]>(initialSlots());

	let selectedProgram = $derived(
		workspace.programs.find((program) => program.id === selectedProgramId) ?? null
	);
	let selectedProgramName = $derived(selectedProgram?.nameTh ?? 'เลือกแผนการเรียน');
	let selectedGradeName = $derived(
		workspace.gradeLevels.find((grade) => grade.id === selectedGradeId)?.name ?? 'เลือกระดับชั้น'
	);
	let selectedTermName = $derived(
		workspace.termSlots.find((slot) => slot.id === selectedTermSlotId)?.name ?? 'เลือกภาคเรียน'
	);
	let selectionReady = $derived(
		workspace.programs.some((program) => program.id === selectedProgramId) &&
			workspace.gradeLevels.some((grade) => grade.id === selectedGradeId) &&
			workspace.termSlots.some((slot) => slot.id === selectedTermSlotId)
	);
	let selectionGuidance = $derived(
		!selectedProgramId
			? 'เพิ่มแผนการเรียนก่อนเพิ่มรายวิชาหรือกิจกรรม'
			: !selectedGradeId
				? 'กำหนดระดับชั้นก่อนเพิ่มรายวิชาหรือกิจกรรม'
				: 'กำหนดภาคเรียนก่อนเพิ่มรายวิชาหรือกิจกรรม'
	);
	let filteredCatalog = $derived.by(() => {
		const query = search.trim().toLocaleLowerCase('th');
		return managementOptions.catalogVersions.filter((option) => {
			if (!selectableCatalogOption(option, selectedGradeId)) return false;
			if (resourceKind !== 'all' && option.section !== resourceKind) return false;
			return !query || `${option.code} ${option.name}`.toLocaleLowerCase('th').includes(query);
		});
	});
	let visibleRequirements = $derived(
		stagedRequirements
			.filter(
				(requirement) =>
					requirement.studyProgramId === selectedProgramId &&
					requirement.gradeLevelId === selectedGradeId &&
					requirement.termSlotId === selectedTermSlotId
			)
			.sort((left, right) => left.displayOrder - right.displayOrder)
	);
	let requirementSections = $derived(
		groupStagedRequirements(
			visibleRequirements,
			managementOptions.catalogVersions,
			workspace.requirements
		)
	);
	let originalProgramRequirements = $derived(
		workspace.requirements.filter((item) => item.studyProgramId === selectedProgramId)
	);
	let stagedProgramRequirements = $derived(
		stagedRequirements.filter((item) => item.studyProgramId === selectedProgramId)
	);
	let addedCount = $derived(
		Math.max(0, stagedProgramRequirements.length - originalProgramRequirements.length)
	);
	let removedCount = $derived(
		Math.max(0, originalProgramRequirements.length - stagedProgramRequirements.length)
	);
	let slotsDirty = $derived(
		JSON.stringify(stagedSlots) !==
			JSON.stringify(
				workspace.termSlots.map((slot) => ({
					id: slot.id,
					sequence: slot.sequence,
					termType: slot.termType,
					typeOccurrence: slot.typeOccurrence,
					name: slot.name
				}))
			)
	);

	function catalogOption(id: string) {
		return managementOptions.catalogVersions.find((option) => option.id === id);
	}

	function requirementSource(id: string) {
		return workspace.requirements.find(
			(source) =>
				source.studyProgramId === selectedProgramId &&
				source.catalogVersionId === id &&
				source.gradeLevel.id === selectedGradeId &&
				source.termSlotId === selectedTermSlotId
		);
	}

	function replaceStaged(next: StagedRequirement[]) {
		history = [...history, stagedRequirements];
		stagedRequirements = next;
	}

	function toggleCatalog(id: string, checked: boolean) {
		selectedCatalogIds = checked
			? [...selectedCatalogIds, id]
			: selectedCatalogIds.filter((value) => value !== id);
	}

	let selectionContext = $derived({
		studyProgramId: selectedProgramId,
		gradeLevelId: selectedGradeId,
		termSlotId: selectedTermSlotId
	});
	let catalogChoices = $derived(
		filteredCatalog.map((option) => ({
			option,
			hasExistingCourse: hasExistingCourseRequirement(
				option,
				selectionContext,
				stagedRequirements,
				managementOptions.catalogVersions,
				workspace.requirements
			)
		}))
	);

	function selectContext(field: 'program' | 'grade' | 'term', value: string) {
		selectedCatalogIds = [];
		if (field === 'program') selectedProgramId = value;
		else if (field === 'grade') selectedGradeId = value;
		else selectedTermSlotId = value;
	}

	function addSelected() {
		if (!selectionReady) return;
		const next = appendSelectedCatalogRequirements(
			stagedRequirements,
			selectedCatalogIds,
			selectionContext,
			managementOptions.catalogVersions,
			workspace.requirements,
			requirementKind
		);
		if (next.length !== stagedRequirements.length) replaceStaged(next);
		selectedCatalogIds = [];
	}

	function updateRequirement(
		target: StagedRequirement,
		values: Partial<CurriculumStructureRequirementInput>
	) {
		replaceStaged(
			stagedRequirements.map((item) => (item === target ? { ...item, ...values } : item))
		);
	}

	function removeRequirement(target: StagedRequirement) {
		replaceStaged(stagedRequirements.filter((item) => item !== target));
	}

	function moveRequirement(target: StagedRequirement, direction: -1 | 1) {
		replaceStaged(
			moveStagedRequirementWithinSection(
				stagedRequirements,
				target,
				direction,
				managementOptions.catalogVersions,
				workspace.requirements
			)
		);
	}
	function undo() {
		const previous = history.at(-1);
		if (!previous) return;
		stagedRequirements = previous;
		history = history.slice(0, -1);
	}

	async function saveStructure() {
		if (!selectedProgram) return;
		saving = true;
		errorMessage = '';
		try {
			if (requirementRowVersions[selectedProgram.id] === undefined)
				throw new Error('ข้อมูลแผนเปลี่ยนไป กรุณาเปิดตัวแก้ไขใหม่เพื่อตรวจสอบ ร่างนี้ยังอยู่');
			await onSaveStructure(
				selectedProgram.id,
				requirementRowVersions[selectedProgram.id],
				stagedProgramRequirements.map(({ studyProgramId: _, ...requirement }) => requirement)
			);
			onClose();
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'บันทึกโครงสร้างหลักสูตรไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}

	async function saveTermSlots() {
		saving = true;
		errorMessage = '';
		try {
			await onSaveTermSlots(slotRowVersion, stagedSlots);
			onClose();
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'บันทึกภาคเรียนไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}

	async function createProgram() {
		if (!programDraft.nameTh.trim()) return;
		saving = true;
		errorMessage = '';
		try {
			if (!workspace.level.draftId) throw new Error('เปิดร่างแก้ไขก่อนเพิ่มแผน');
			await onCreateProgram({
				draftId: workspace.level.draftId,
				nameTh: programDraft.nameTh.trim()
			});
			onClose();
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'เพิ่มแผนการเรียนไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}

	function handleOpenChange(value: boolean) {
		open = value;
		if (!value) onClose();
	}
</script>

<Sheet.Root bind:open onOpenChange={handleOpenChange}>
	<Sheet.Content class="w-full overflow-y-auto sm:max-w-6xl">
		<Sheet.Header class="pe-8 text-start">
			<div class="flex flex-wrap items-center justify-between gap-2">
				<Sheet.Title>จัดโครงสร้างหลักสูตร</Sheet.Title>
				<Button variant="outline" size="sm" onclick={() => (settingsOpen = !settingsOpen)}
					><Settings class="size-4" />{settingsOpen ? 'กลับไปจัดรายวิชา' : 'ตั้งค่า'}</Button
				>
			</div>
			<Sheet.Description>
				เพิ่ม ย้าย และนำรายการออกจากแผนแบบร่าง ค่าหน่วยกิตและชั่วโมงอ่านจากทะเบียนเท่านั้น
			</Sheet.Description>
		</Sheet.Header>

		<div class="space-y-5 py-4">
			{#if settingsOpen}
				<section class="space-y-3 rounded-xl border bg-card p-3" aria-label="เพิ่มแผนการเรียน">
					<h3 class="font-semibold">เพิ่มแผนการเรียน</h3>
					<Label for="settings-program-name">ชื่อแผนการเรียนใหม่</Label>
					<div class="flex flex-wrap gap-2">
						<Input
							id="settings-program-name"
							bind:value={programDraft.nameTh}
							placeholder="ชื่อแผนการเรียน"
							class="min-w-48 flex-1"
						/><LoadingButton loading={saving} disabled={orderSaving} onclick={createProgram}
							>เพิ่มแผน</LoadingButton
						>
					</div>
				</section>
				<section class="space-y-3 rounded-xl border bg-card p-3" aria-label="ลำดับแผนการเรียน">
					<h3 class="font-semibold">ลำดับแผนการเรียน</h3>
					<p class="text-xs text-muted-foreground">
						เรียงแผนที่ 1, 2, 3 ตามลำดับที่ต้องการ แล้วบันทึกครั้งเดียว
					</p>
					<ol class="divide-y rounded-lg border">
						{#each orderIds as id, index (id)}
							<li class="flex items-center gap-2 p-2" data-program-order-id={id}>
								<span class="w-6 text-center text-sm tabular-nums">{index + 1}</span><span
									class="min-w-0 flex-1 text-sm"
									>{workspace.programs.find((program) => program.id === id)?.nameTh ??
										'ไม่พบแผนในข้อมูลล่าสุด'}</span
								>
								<Button
									variant="ghost"
									size="icon"
									disabled={index === 0 || orderSaving}
									onclick={() => moveProgram(index, -1)}
									aria-label={`เลื่อนแผน ${index + 1} ขึ้น`}><ArrowUp class="size-4" /></Button
								>
								<Button
									variant="ghost"
									size="icon"
									disabled={index === orderIds.length - 1 || orderSaving}
									onclick={() => moveProgram(index, 1)}
									aria-label={`เลื่อนแผน ${index + 1} ลง`}><ArrowDown class="size-4" /></Button
								>
							</li>
						{/each}
					</ol>
					{#if orderError}<p role="alert" class="text-sm text-destructive">{orderError}</p>{/if}
					{#if orderConflict}<p class="text-sm text-muted-foreground">
							ร่างลำดับและรายวิชายังอยู่ โหลดลำดับล่าสุดแล้วตรวจสอบก่อนบันทึกอีกครั้ง
						</p>
						<LoadingButton variant="outline" loading={orderSaving} onclick={reviewProgramOrder}
							>โหลดลำดับล่าสุดเพื่อตรวจสอบ</LoadingButton
						>{/if}
					{#if !orderComplete}<p class="text-sm text-destructive">
							รายการแผนเปลี่ยนไป ใช้ลำดับปัจจุบันก่อนจัดลำดับใหม่
						</p>{/if}
					<p class="text-xs text-muted-foreground">
						ลำดับปัจจุบัน: {orderedPrograms.map((program) => program.nameTh).join(' → ')}
					</p>
					<div class="flex flex-wrap justify-end gap-2">
						<Button variant="outline" disabled={orderSaving} onclick={resetOrder}
							>ใช้ลำดับปัจจุบัน</Button
						><LoadingButton
							loading={orderSaving}
							disabled={saving || !orderDirty || !orderComplete || orderConflict}
							onclick={saveProgramOrder}>บันทึกลำดับแผน</LoadingButton
						>
					</div>
				</section>
				<CurriculumCreditCriteriaSettings
					criteria={criteriaDraft}
					currentCriteria={criteriaBaseline}
					grades={workspace.gradeLevels}
					coveredGradeIds={workspace.level.gradeLevelIds}
					saving={criteriaSaving}
					dirty={criteriaDirty}
					error={criteriaError}
					conflict={criteriaConflict}
					onchange={(criteria) => (criteriaDraft = criteria)}
					onsave={saveCreditCriteria}
					onreview={reviewCreditCriteria}
					onreset={() => {
						criteriaDraft = structuredClone(criteriaBaseline);
						criteriaError = '';
					}}
				/>
				<CurriculumTermSlotEditor slots={stagedSlots} onchange={(slots) => (stagedSlots = slots)} />
				{#if slotsDirty}<div
						class="flex flex-wrap items-center justify-between gap-2 rounded-lg border bg-muted p-3 text-sm"
					>
						<span>บันทึกภาคเรียนก่อน แล้วเปิดตัวแก้ไขอีกครั้งเพื่อใช้ช่องภาคเรียนใหม่</span
						><LoadingButton
							loading={saving}
							disabled={orderSaving || orderConflict}
							onclick={saveTermSlots}>บันทึกภาคเรียน</LoadingButton
						>
					</div>{/if}
			{:else}
				<div class="grid gap-3 rounded-xl border bg-card p-3 lg:grid-cols-3">
					<Select.Root
						type="single"
						value={selectedProgramId}
						onValueChange={(value) => selectContext('program', value)}
					>
						<Select.Trigger aria-label="แผนการเรียนที่จะจัดโครงสร้าง"
							>{selectedProgramName}</Select.Trigger
						>
						<Select.Content>
							{#each orderedPrograms as program (program.id)}
								<Select.Item value={program.id}>{program.nameTh}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
					<Select.Root
						type="single"
						value={selectedGradeId}
						onValueChange={(value) => selectContext('grade', value)}
					>
						<Select.Trigger aria-label="ระดับชั้นที่จะจัดโครงสร้าง"
							>{selectedGradeName}</Select.Trigger
						>
						<Select.Content>
							{#each workspace.gradeLevels as grade (grade.id)}
								<Select.Item value={grade.id}>{grade.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
					<Select.Root
						type="single"
						value={selectedTermSlotId}
						onValueChange={(value) => selectContext('term', value)}
					>
						<Select.Trigger aria-label="ภาคเรียนที่จะจัดโครงสร้าง"
							>{selectedTermName}</Select.Trigger
						>
						<Select.Content>
							{#each workspace.termSlots as slot (slot.id)}
								<Select.Item value={slot.id}>{slot.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				{#if !selectionReady}
					<p role="status" class="text-sm text-amber-700 dark:text-amber-300">
						{selectionGuidance}
					</p>
				{/if}

				<div class="grid min-w-0 gap-4 xl:grid-cols-[minmax(18rem,0.8fr)_minmax(0,1.4fr)]">
					<section class="min-w-0 space-y-3 rounded-xl border p-3" aria-label="เลือกจากทะเบียน">
						<div>
							<h3 class="font-semibold">เลือกจากทะเบียน</h3>
							<p class="text-xs text-muted-foreground">
								เลือกรายการได้หลายรายการ แล้วเพิ่มพร้อมกัน
							</p>
						</div>
						<div class="relative">
							<Search class="absolute left-3 top-2.5 size-4 text-muted-foreground" />
							<Input bind:value={search} class="pl-9" placeholder="ค้นหารหัสหรือชื่อ" />
						</div>
						<Select.Root
							type="single"
							value={resourceKind}
							onValueChange={(value) => {
								if (
									value === 'all' ||
									value === 'basic_course' ||
									value === 'additional_course' ||
									value === 'student_development'
								) {
									resourceKind = value;
									selectedCatalogIds = [];
								}
							}}
						>
							<Select.Trigger aria-label="หมวดทะเบียน">
								{resourceKind === 'all'
									? 'ทั้งหมด'
									: resourceKind === 'basic_course'
										? 'รายวิชาพื้นฐาน'
										: resourceKind === 'additional_course'
											? 'รายวิชาเพิ่มเติม'
											: 'กิจกรรมพัฒนาผู้เรียน'}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="all">ทั้งหมด</Select.Item>
								<Select.Item value="basic_course">รายวิชาพื้นฐาน</Select.Item>
								<Select.Item value="additional_course">รายวิชาเพิ่มเติม</Select.Item>
								<Select.Item value="student_development">กิจกรรมพัฒนาผู้เรียน</Select.Item>
							</Select.Content>
						</Select.Root>
						<div class="max-h-72 space-y-1 overflow-y-auto rounded-lg border p-1">
							{#each catalogChoices as { option, hasExistingCourse } (option.id)}
								<label
									class="flex cursor-pointer items-start gap-2 rounded-md p-2 hover:bg-muted/60"
								>
									<Checkbox
										aria-label={`เลือก ${option.resourceKind === 'course' ? option.code + ' ' : ''}${option.name} · ${formatEffectiveRange(option.effectiveFrom, option.effectiveUntil)}`}
										disabled={hasExistingCourse || !selectionReady || saving}
										checked={selectedCatalogIds.includes(option.id)}
										onCheckedChange={(checked) => toggleCatalog(option.id, checked === true)}
									/>
									<span class="min-w-0">
										<span class="block text-xs font-semibold text-primary tabular-nums"
											>{option.resourceKind === 'course' ? option.code : ''}</span
										>
										<span class="block text-sm">{option.name}</span>
										<span class="block text-xs text-muted-foreground"
											>{formatEffectiveRange(option.effectiveFrom, option.effectiveUntil)}</span
										>
										{#if hasExistingCourse}<span class="block text-xs text-muted-foreground"
												>มีรายวิชานี้แล้ว</span
											>{/if}
									</span>
								</label>
							{/each}
						</div>
						<Select.Root type="single" bind:value={requirementKind}>
							<Select.Trigger>
								{requirementKind === 'required'
									? 'บังคับ'
									: requirementKind === 'elective'
										? 'เลือก'
										: 'เพิ่มเติม'}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="required">บังคับ</Select.Item>
								<Select.Item value="elective">เลือก</Select.Item>
								<Select.Item value="optional">เพิ่มเติม</Select.Item>
							</Select.Content>
						</Select.Root>
						<Button
							class="w-full"
							onclick={addSelected}
							disabled={!selectionReady || selectedCatalogIds.length === 0}
						>
							<ArrowDownToLine class="size-4" /> เพิ่ม {selectedCatalogIds.length || ''} รายการ
						</Button>
					</section>

					<section class="min-w-0 overflow-hidden rounded-xl border" aria-label="รายการในหลักสูตร">
						<div class="flex items-center justify-between border-b bg-muted/30 px-3 py-2">
							<div>
								<h3 class="font-semibold">รายการใน {selectedTermName}</h3>
								<p class="text-xs text-muted-foreground">
									{selectedProgramName} · {selectedGradeName}
								</p>
							</div>
							<Button variant="ghost" size="sm" onclick={undo} disabled={history.length === 0}>
								<RotateCcw class="size-3.5" /> ย้อนกลับ
							</Button>
						</div>
						<div class="overflow-x-auto">
							<Table.Root class="min-w-[680px]">
								<Table.Header>
									<Table.Row>
										<Table.Head>รายการ</Table.Head>
										<Table.Head class="w-44">เงื่อนไข</Table.Head>
										<Table.Head class="w-40">ค่าทางการ</Table.Head>
										<Table.Head class="w-24">ลำดับ</Table.Head><Table.Head class="w-12"
										></Table.Head>
									</Table.Row>
								</Table.Header>
								<Table.Body>
									{#each requirementSections as section (section.id)}
										{#if section.id !== 'unclassified' || section.rows.length}<Table.Row
												class="bg-muted/40"
												><Table.Cell colspan={5} class="font-semibold">{section.label}</Table.Cell
												></Table.Row
											>{/if}
										{#each section.rows as requirement, rowIndex (requirement)}
											{@const option = catalogOption(requirement.catalogVersionId)}
											{@const source = requirementSource(requirement.catalogVersionId)}
											<Table.Row>
												<Table.Cell>
													<div class="text-xs font-semibold text-primary tabular-nums">
														{requirement.resourceKind === 'course'
															? (source?.code ?? option?.code)
															: ''}
													</div>
													<div class="font-medium">
														{source?.name ?? option?.name ?? 'ไม่มีข้อมูลชื่อในทะเบียน'}
													</div>
													<CurriculumOutdatedBadge
														hasNewerCurrentVersion={source?.hasNewerCurrentVersion ?? false}
													/>
												</Table.Cell>
												<Table.Cell>
													<Select.Root
														type="single"
														value={requirement.requirementKind}
														onValueChange={(value) =>
															value &&
															isRequirementKind(value) &&
															updateRequirement(requirement, {
																requirementKind: value
															})}
													>
														<Select.Trigger>
															{requirement.requirementKind === 'required'
																? 'บังคับ'
																: requirement.requirementKind === 'elective'
																	? 'เลือก'
																	: 'เพิ่มเติม'}
														</Select.Trigger>
														<Select.Content>
															<Select.Item value="required">บังคับ</Select.Item>
															<Select.Item value="elective">เลือก</Select.Item>
															<Select.Item value="optional">เพิ่มเติม</Select.Item>
														</Select.Content>
													</Select.Root>
												</Table.Cell>
												<Table.Cell class="text-xs text-muted-foreground">
													{#if source}
														{source.metrics.credit
															? `${source.metrics.credit} หน่วยกิต · `
															: ''}{source.metrics.totalHours ?? '—'} ชม.
													{:else}
														จากทะเบียนเมื่อบันทึก
													{/if}
												</Table.Cell>
												<Table.Cell
													><div class="flex">
														<Button
															variant="ghost"
															size="icon"
															disabled={rowIndex === 0}
															onclick={() => moveRequirement(requirement, -1)}
															aria-label="เลื่อนรายการขึ้น"><ArrowUp class="size-4" /></Button
														><Button
															variant="ghost"
															size="icon"
															disabled={rowIndex === section.rows.length - 1}
															onclick={() => moveRequirement(requirement, 1)}
															aria-label="เลื่อนรายการลง"><ArrowDown class="size-4" /></Button
														>
													</div></Table.Cell
												>
												<Table.Cell>
													<Button
														variant="ghost"
														size="icon"
														onclick={() => removeRequirement(requirement)}
														aria-label="นำรายการออก"
													>
														<Trash2 class="size-4" />
													</Button>
												</Table.Cell>
											</Table.Row>
										{/each}{/each}
								</Table.Body>
							</Table.Root>
						</div>
					</section>
				</div>

				<div
					class="flex flex-col gap-3 rounded-xl border bg-muted/20 p-3 sm:flex-row sm:items-center"
				>
					<div class="flex flex-1 flex-wrap gap-2 text-sm">
						<Badge variant="secondary">เพิ่ม {addedCount}</Badge>
						<Badge variant="secondary">นำออก {removedCount}</Badge>
						<span class="text-muted-foreground">ตรวจรายการแล้วบันทึกครั้งเดียว</span>
					</div>
					<LoadingButton
						loading={saving}
						onclick={saveStructure}
						disabled={!selectedProgram || slotsDirty || orderSaving || orderConflict}
					>
						<Save class="size-4" /> บันทึกโครงสร้าง
					</LoadingButton>
				</div>
			{/if}
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		</div>
	</Sheet.Content>
</Sheet.Root>
