<script lang="ts">
	import type {
		CreateStudyProgramRequest,
		CurriculumManagementOptions,
		CurriculumStructureRequirementInput,
		CurriculumStructureWorkspace,
		CurriculumTermSlotInput
	} from '$lib/api/academic-core';
	import { LoadingButton } from '$lib/components/app-state';
	import CurriculumTermSlotEditor from '$lib/components/academic-core/CurriculumTermSlotEditor.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
	import * as Sheet from '$lib/components/ui/sheet';
	import * as Table from '$lib/components/ui/table';
	import { ArrowDownToLine, RotateCcw, Save, Search, Trash2 } from 'lucide-svelte';

	let {
		workspace,
		managementOptions,
		onSaveStructure,
		onSaveTermSlots,
		onCreateProgram,
		onClose
	}: {
		workspace: CurriculumStructureWorkspace;
		managementOptions: CurriculumManagementOptions;
		onSaveStructure: (
			studyProgramId: string,
			rowVersion: number,
			requirements: CurriculumStructureRequirementInput[]
		) => Promise<void>;
		onSaveTermSlots: (slots: CurriculumTermSlotInput[]) => Promise<void>;
		onCreateProgram: (request: CreateStudyProgramRequest) => Promise<void>;
		onClose: () => void;
	} = $props();
	type StagedRequirement = CurriculumStructureRequirementInput & { studyProgramId: string };
	function initialProgramId() {
		return workspace.programs[0]?.id ?? '';
	}
	function initialGradeId() {
		return workspace.gradeLevels[0]?.id ?? '';
	}
	function initialTermSlotId() {
		return workspace.termSlots[0]?.id ?? '';
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
	let resourceKind = $state<'all' | 'course' | 'activity'>('all');
	let search = $state('');
	let selectedCatalogIds = $state.raw<string[]>([]);
	let programDraft = $state({ code: '', nameTh: '', isDefault: initialProgramId() === '' });
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
		Boolean(selectedProgramId && selectedGradeId && selectedTermSlotId)
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
			if (resourceKind !== 'all' && option.resourceKind !== resourceKind) return false;
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
			(source) => source.studyProgramId === selectedProgramId && source.catalogVersionId === id
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

	function addSelected() {
		if (!selectionReady) return;
		const next = [...stagedRequirements];
		let order =
			Math.max(
				0,
				...next
					.filter(
						(item) =>
							item.gradeLevelId === selectedGradeId && item.termSlotId === selectedTermSlotId
					)
					.map((item) => item.displayOrder)
			) + 1;
		for (const catalogVersionId of selectedCatalogIds) {
			const option = catalogOption(catalogVersionId);
			if (!option) continue;
			const duplicate = next.some(
				(item) =>
					item.studyProgramId === selectedProgramId &&
					item.catalogVersionId === catalogVersionId &&
					item.gradeLevelId === selectedGradeId &&
					item.termSlotId === selectedTermSlotId
			);
			if (duplicate) continue;
			next.push({
				studyProgramId: selectedProgramId,
				resourceKind: option.resourceKind,
				catalogVersionId,
				gradeLevelId: selectedGradeId,
				termSlotId: selectedTermSlotId,
				requirementKind,
				displayOrder: order++
			});
		}
		replaceStaged(next);
		selectedCatalogIds = [];
	}

	function updateRequirement(
		catalogVersionId: string,
		values: Partial<CurriculumStructureRequirementInput>
	) {
		replaceStaged(
			stagedRequirements.map((item) =>
				item.studyProgramId === selectedProgramId &&
				item.catalogVersionId === catalogVersionId &&
				item.gradeLevelId === selectedGradeId &&
				item.termSlotId === selectedTermSlotId
					? { ...item, ...values }
					: item
			)
		);
	}

	function removeRequirement(catalogVersionId: string) {
		replaceStaged(
			stagedRequirements.filter(
				(item) =>
					!(
						item.studyProgramId === selectedProgramId &&
						item.catalogVersionId === catalogVersionId &&
						item.gradeLevelId === selectedGradeId &&
						item.termSlotId === selectedTermSlotId
					)
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
			await onSaveStructure(
				selectedProgram.id,
				selectedProgram.rowVersion,
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
			await onSaveTermSlots(stagedSlots);
			onClose();
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'บันทึกภาคเรียนไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}

	async function createProgram() {
		if (!programDraft.code.trim() || !programDraft.nameTh.trim()) return;
		saving = true;
		errorMessage = '';
		try {
			await onCreateProgram({
				code: programDraft.code.trim(),
				nameTh: programDraft.nameTh.trim(),
				nameEn: null,
				isDefault: programDraft.isDefault,
				owningOrganizationUnitId: null
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
			<Sheet.Title>จัดโครงสร้างหลักสูตร</Sheet.Title>
			<Sheet.Description>
				เพิ่ม ย้าย และนำรายการออกจากแผนแบบร่าง ค่าหน่วยกิตและชั่วโมงอ่านจากทะเบียนเท่านั้น
			</Sheet.Description>
		</Sheet.Header>

		<div class="space-y-5 py-4">
			<section
				class="grid gap-2 rounded-xl border bg-card p-3 sm:grid-cols-[10rem_minmax(14rem,1fr)_auto_auto] sm:items-center"
			>
				<Input bind:value={programDraft.code} placeholder="รหัสแผน เช่น GENERAL" />
				<Input bind:value={programDraft.nameTh} placeholder="ชื่อแผนการเรียน" />
				<label class="flex items-center gap-2 text-sm">
					<Checkbox bind:checked={programDraft.isDefault} /> แผนเริ่มต้น
				</label>
				<LoadingButton loading={saving} onclick={createProgram}>เพิ่มแผน</LoadingButton>
			</section>

			<CurriculumTermSlotEditor slots={stagedSlots} onchange={(slots) => (stagedSlots = slots)} />
			{#if slotsDirty}
				<div
					class="flex items-center justify-between rounded-lg border border-amber-300 bg-amber-50 p-3 text-sm dark:border-amber-900 dark:bg-amber-950/20"
				>
					<span>บันทึกภาคเรียนก่อน แล้วเปิดตัวแก้ไขอีกครั้งเพื่อใช้ช่องภาคเรียนใหม่</span>
					<LoadingButton loading={saving} onclick={saveTermSlots}>บันทึกภาคเรียน</LoadingButton>
				</div>
			{/if}

			<div class="grid gap-3 rounded-xl border bg-card p-3 lg:grid-cols-3">
				<Select.Root type="single" bind:value={selectedProgramId}>
					<Select.Trigger>{selectedProgramName}</Select.Trigger>
					<Select.Content>
						{#each workspace.programs as program (program.id)}
							<Select.Item value={program.id}>{program.nameTh}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
				<Select.Root type="single" bind:value={selectedGradeId}>
					<Select.Trigger>{selectedGradeName}</Select.Trigger>
					<Select.Content>
						{#each workspace.gradeLevels as grade (grade.id)}
							<Select.Item value={grade.id}>{grade.name}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
				<Select.Root type="single" bind:value={selectedTermSlotId}>
					<Select.Trigger>{selectedTermName}</Select.Trigger>
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

			<div class="grid gap-4 xl:grid-cols-[minmax(18rem,0.8fr)_minmax(34rem,1.4fr)]">
				<section class="space-y-3 rounded-xl border p-3">
					<div>
						<h3 class="font-semibold">เลือกจากทะเบียน</h3>
						<p class="text-xs text-muted-foreground">เลือกรายการได้หลายรายการ แล้วเพิ่มพร้อมกัน</p>
					</div>
					<div class="relative">
						<Search class="absolute left-3 top-2.5 size-4 text-muted-foreground" />
						<Input bind:value={search} class="pl-9" placeholder="ค้นหารหัสหรือชื่อ" />
					</div>
					<Select.Root type="single" bind:value={resourceKind}>
						<Select.Trigger>
							{resourceKind === 'all'
								? 'ทั้งหมด'
								: resourceKind === 'course'
									? 'รายวิชา'
									: 'กิจกรรม'}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="all">ทั้งหมด</Select.Item>
							<Select.Item value="course">รายวิชา</Select.Item>
							<Select.Item value="activity">กิจกรรม</Select.Item>
						</Select.Content>
					</Select.Root>
					<div class="max-h-72 space-y-1 overflow-y-auto rounded-lg border p-1">
						{#each filteredCatalog as option (option.id)}
							<label class="flex cursor-pointer items-start gap-2 rounded-md p-2 hover:bg-muted/60">
								<Checkbox
									checked={selectedCatalogIds.includes(option.id)}
									onCheckedChange={(checked) => toggleCatalog(option.id, checked === true)}
								/>
								<span class="min-w-0">
									<span class="block font-mono text-xs font-semibold text-primary"
										>{option.code}</span
									>
									<span class="block text-sm">{option.name}</span>
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

				<section class="overflow-hidden rounded-xl border">
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
									<Table.Head class="w-12"></Table.Head>
								</Table.Row>
							</Table.Header>
							<Table.Body>
								{#each visibleRequirements as requirement (requirement.catalogVersionId)}
									{@const option = catalogOption(requirement.catalogVersionId)}
									{@const source = requirementSource(requirement.catalogVersionId)}
									<Table.Row>
										<Table.Cell>
											<div class="font-mono text-xs font-semibold text-primary">
												{option?.code ?? source?.code}
											</div>
											<div class="font-medium">{option?.name ?? source?.name}</div>
										</Table.Cell>
										<Table.Cell>
											<Select.Root
												type="single"
												value={requirement.requirementKind}
												onValueChange={(value) =>
													value &&
													isRequirementKind(value) &&
													updateRequirement(requirement.catalogVersionId, {
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
												{source.metrics.credit ? `${source.metrics.credit} หน่วยกิต · ` : ''}{source
													.metrics.totalHours ?? '—'} ชม.
											{:else}
												จากทะเบียนเมื่อบันทึก
											{/if}
										</Table.Cell>
										<Table.Cell>
											<Button
												variant="ghost"
												size="icon"
												onclick={() => removeRequirement(requirement.catalogVersionId)}
												aria-label="นำรายการออก"
											>
												<Trash2 class="size-4" />
											</Button>
										</Table.Cell>
									</Table.Row>
								{/each}
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
					disabled={!selectedProgram || slotsDirty}
				>
					<Save class="size-4" /> บันทึกโครงสร้าง
				</LoadingButton>
			</div>
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		</div>
	</Sheet.Content>
</Sheet.Root>
