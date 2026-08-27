<script lang="ts">
	import { SvelteMap } from 'svelte/reactivity';
	import type {
		CreateStudyProgramRequest,
		CurriculumManagementOptions,
		CurriculumRequirementView,
		CurriculumVersion,
		ProgramRequirementInput,
		StudyProgram
	} from '$lib/api/academic-core';
	import { LoadingButton } from '$lib/components/app-state';
	import AcademicPrerequisiteNotice from '$lib/components/academic-workflow/AcademicPrerequisiteNotice.svelte';
	import type { AcademicPrerequisite } from '$lib/components/academic-workflow/prerequisite';
	import { Badge } from '$lib/components/ui/badge';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { BookCheck, CheckCircle2, Layers3, Plus, Settings2, Trash2 } from 'lucide-svelte';

	let {
		version,
		programs,
		requirements,
		managementOptions,
		canManage = false,
		onRequestManagementOptions,
		onCreateProgram,
		onReplaceRequirements,
		onPublishVersion
	}: {
		version: CurriculumVersion;
		programs: StudyProgram[];
		requirements: CurriculumRequirementView[];
		managementOptions: CurriculumManagementOptions | null;
		canManage?: boolean;
		onRequestManagementOptions: () => Promise<CurriculumManagementOptions | null>;
		onCreateProgram: (draft: CreateStudyProgramRequest) => Promise<void>;
		onReplaceRequirements: (
			program: StudyProgram,
			requirements: ProgramRequirementInput[]
		) => Promise<void>;
		onPublishVersion: (id: string, rowVersion: number) => Promise<void>;
	} = $props();

	const unspecifiedTerm = 'unspecified';
	const requirementKindLabels = {
		required: 'บังคับ',
		elective: 'เลือก',
		optional: 'เพิ่มเติม'
	} as const;
	const termLabels: Record<string, string> = {
		'1': 'ภาคเรียนที่ 1',
		'2': 'ภาคเรียนที่ 2',
		summer: 'ภาคฤดูร้อน'
	};

	let managementOpen = $state(false);
	let managementLoading = $state(false);
	let createProgramBusy = $state(false);
	let requirementBusy = $state(false);
	let publishing = $state(false);
	let removingRequirementId = $state('');
	let errorMessage = $state('');
	let programDraft = $state({ code: '', nameTh: '', nameEn: '', isDefault: false });
	let requirementProgramId = $state('');
	let requirementProgramError = $state('');
	let requirementProgramTrigger = $state<HTMLButtonElement | null>(null);
	let requirementDraft = $state({
		resourceKind: 'course' as 'course' | 'activity',
		catalogVersionId: '',
		gradeLevelId: '',
		requirementKind: 'required' as 'required' | 'elective' | 'optional',
		recommendedTermCode: unspecifiedTerm,
		credit: '',
		hours: ''
	});

	let gradeLevels = $derived(managementOptions?.gradeLevels ?? []);
	let catalogVersions = $derived(
		(managementOptions?.catalogVersions ?? []).filter(
			(option) => option.resourceKind === requirementDraft.resourceKind
		)
	);

	const noPrograms: AcademicPrerequisite = {
		key: 'curriculum-study-program',
		status: 'missing',
		title: 'เพิ่มแผนการเรียนก่อน',
		description: 'รายการรายวิชาและกิจกรรมต้องอยู่ภายใต้แผนการเรียนที่ชัดเจน'
	};
	const noCatalogVersions: AcademicPrerequisite = {
		key: 'curriculum-catalog-version',
		status: 'missing',
		title: 'ยังไม่มีรายวิชาหรือกิจกรรมที่เผยแพร่',
		description: 'เผยแพร่รุ่นรายวิชาหรือกิจกรรมในทะเบียนก่อน แล้วจึงกลับมาเพิ่มในแผนการเรียน'
	};
	const noGradeLevels: AcademicPrerequisite = {
		key: 'curriculum-requirement-grade',
		status: 'missing',
		title: 'ยังไม่มีระดับชั้นให้เลือก',
		description: 'กรุณาติดต่อผู้ดูแลระบบให้ตั้งค่าระดับชั้นก่อนเพิ่มรายการในแผนการเรียน'
	};

	function programRequirements(programId: string) {
		return requirements.filter((item) => item.studyProgramId === programId);
	}

	function requirementGroups(programId: string) {
		const groups = new SvelteMap<
			string,
			{ gradeName: string; termName: string; items: CurriculumRequirementView[] }
		>();
		for (const item of programRequirements(programId).sort(
			(left, right) =>
				left.gradeLevel.level_order - right.gradeLevel.level_order ||
				left.requirement.displayOrder - right.requirement.displayOrder
		)) {
			const termCode = item.requirement.recommendedTermCode ?? unspecifiedTerm;
			const key = `${item.gradeLevel.id}:${termCode}`;
			const group = groups.get(key) ?? {
				gradeName: item.gradeLevel.name,
				termName: termLabels[termCode] ?? 'ไม่ระบุภาคเรียน',
				items: []
			};
			group.items.push(item);
			groups.set(key, group);
		}
		return [...groups.values()];
	}

	function requirementInput(view: CurriculumRequirementView, displayOrder: number) {
		return {
			catalogVersionId: view.requirement.catalogVersionId,
			gradeLevelId: view.requirement.gradeLevelId,
			resourceKind: view.requirement.resourceKind,
			requirementKind: view.requirement.requirementKind,
			credit: view.requirement.credit ?? null,
			hours: view.requirement.hours ?? null,
			recommendedTermCode: view.requirement.recommendedTermCode ?? null,
			displayOrder
		} satisfies ProgramRequirementInput;
	}

	function catalogLabel(optionId: string) {
		const option = catalogVersions.find((candidate) => candidate.id === optionId);
		if (!option) return 'เลือกรายวิชาหรือกิจกรรม';
		return `${option.code} · ${option.name} · รุ่น ${option.versionNo}`;
	}

	async function openManagement() {
		managementLoading = true;
		errorMessage = '';
		try {
			await onRequestManagementOptions();
			managementOpen = true;
		} catch (error) {
			errorMessage =
				error instanceof Error ? error.message : 'โหลดตัวเลือกสำหรับจัดการหลักสูตรไม่สำเร็จ';
		} finally {
			managementLoading = false;
		}
	}

	async function createProgram(event: SubmitEvent) {
		event.preventDefault();
		createProgramBusy = true;
		errorMessage = '';
		try {
			await onCreateProgram({
				code: programDraft.code.trim(),
				nameTh: programDraft.nameTh.trim(),
				nameEn: programDraft.nameEn.trim() || null,
				isDefault: programDraft.isDefault,
				owningOrganizationUnitId: null
			});
			programDraft = { code: '', nameTh: '', nameEn: '', isDefault: false };
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างแผนการเรียนไม่สำเร็จ';
		} finally {
			createProgramBusy = false;
		}
	}

	async function addRequirement(event: SubmitEvent) {
		event.preventDefault();
		const program = programs.find((item) => item.id === requirementProgramId);
		if (!program) {
			requirementProgramError = 'กรุณาเลือกแผนการเรียนก่อนเพิ่มรายการ';
			requestAnimationFrame(() => requirementProgramTrigger?.focus());
			return;
		}
		requirementProgramError = '';
		const existing = programRequirements(program.id).map((item, index) =>
			requirementInput(item, index + 1)
		);
		const next = [
			...existing,
			{
				catalogVersionId: requirementDraft.catalogVersionId,
				gradeLevelId: requirementDraft.gradeLevelId,
				resourceKind: requirementDraft.resourceKind,
				requirementKind: requirementDraft.requirementKind,
				credit: requirementDraft.credit.trim() || null,
				hours: requirementDraft.hours.trim() || null,
				recommendedTermCode:
					requirementDraft.recommendedTermCode === unspecifiedTerm
						? null
						: requirementDraft.recommendedTermCode,
				displayOrder: existing.length + 1
			} satisfies ProgramRequirementInput
		];
		requirementBusy = true;
		errorMessage = '';
		try {
			await onReplaceRequirements(program, next);
			requirementDraft.catalogVersionId = '';
			requirementDraft.credit = '';
			requirementDraft.hours = '';
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'เพิ่มรายการในแผนการเรียนไม่สำเร็จ';
		} finally {
			requirementBusy = false;
		}
	}

	async function removeRequirement(program: StudyProgram, target: CurriculumRequirementView) {
		const next = programRequirements(program.id)
			.filter((item) => item.requirement.id !== target.requirement.id)
			.map((item, index) => requirementInput(item, index + 1));
		removingRequirementId = target.requirement.id;
		errorMessage = '';
		try {
			await onReplaceRequirements(program, next);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'นำรายการออกจากแผนไม่สำเร็จ';
		} finally {
			removingRequirementId = '';
		}
	}

	async function publish() {
		publishing = true;
		errorMessage = '';
		try {
			await onPublishVersion(version.id, version.rowVersion);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'เผยแพร่รุ่นหลักสูตรไม่สำเร็จ';
		} finally {
			publishing = false;
		}
	}
</script>

<section class="overflow-hidden rounded-2xl border bg-card shadow-sm">
	<header
		class="flex flex-col gap-3 border-b bg-muted/25 p-5 sm:flex-row sm:items-center sm:justify-between"
	>
		<div class="flex items-center gap-3">
			<div class="rounded-xl bg-primary/10 p-2.5 text-primary"><BookCheck class="size-5" /></div>
			<div>
				<div class="flex flex-wrap items-center gap-2">
					<h2 class="font-semibold">{version.versionName}</h2>
					<Badge variant={version.status === 'published' ? 'default' : 'secondary'}>
						{version.status === 'published'
							? 'เผยแพร่แล้ว'
							: version.status === 'archived'
								? 'เก็บถาวร'
								: 'แบบร่าง'}
					</Badge>
				</div>
				<p class="text-xs text-muted-foreground">แผนการเรียนและรายการที่ใช้ในรุ่นนี้</p>
			</div>
		</div>
		{#if canManage && version.status === 'draft'}
			<div class="flex flex-wrap gap-2">
				{#if !managementOpen}
					<LoadingButton
						variant="outline"
						loading={managementLoading}
						loadingLabel="กำลังโหลด"
						onclick={openManagement}
					>
						<Settings2 class="size-4" /> จัดการแบบร่าง
					</LoadingButton>
				{/if}
				<LoadingButton
					loading={publishing}
					loadingLabel="กำลังเผยแพร่"
					disabled={programs.length === 0}
					onclick={publish}
				>
					<CheckCircle2 class="size-4" /> ตรวจสรุปและเผยแพร่
				</LoadingButton>
			</div>
		{/if}
	</header>

	<div class="grid gap-5 p-5 xl:grid-cols-[minmax(0,1fr)_340px]">
		<div class="space-y-4">
			{#each programs as program (program.id)}
				<article class="overflow-hidden rounded-xl border">
					<header class="flex items-center justify-between gap-3 border-b bg-muted/20 px-4 py-3">
						<div>
							<h3 class="font-medium">{program.nameTh}</h3>
							<p class="text-xs text-muted-foreground">
								{program.code}{program.nameEn ? ` · ${program.nameEn}` : ''}
							</p>
						</div>
						{#if program.isDefault}<Badge>แผนเริ่มต้น</Badge>{/if}
					</header>
					{#each requirementGroups(program.id) as group (`${program.id}:${group.gradeName}:${group.termName}`)}
						<div class="border-b bg-muted/10 px-4 py-2 last:border-b-0">
							<p class="text-xs font-semibold text-muted-foreground">
								{group.gradeName} · {group.termName}
							</p>
							<div class="mt-2 divide-y rounded-lg border bg-background">
								{#each group.items as item (item.requirement.id)}
									<div class="flex items-start justify-between gap-3 px-3 py-3 text-sm">
										<div class="flex min-w-0 items-start gap-2">
											<Layers3 class="mt-0.5 size-4 shrink-0 text-muted-foreground" />
											<div class="min-w-0">
												<p class="font-medium">
													<span class="font-mono text-primary">{item.catalog.code}</span>
													· {item.catalog.name}
												</p>
												<p class="text-xs text-muted-foreground">
													{item.catalog.resourceKind === 'course' ? 'รายวิชา' : 'กิจกรรม'} · รุ่น
													{item.catalog.versionNo} · {requirementKindLabels[
														item.requirement.requirementKind
													]}
												</p>
											</div>
										</div>
										<div class="flex shrink-0 items-center gap-2">
											<span class="font-medium tabular-nums">
												{item.requirement.credit
													? `${item.requirement.credit} หน่วยกิต`
													: item.requirement.hours
														? `${item.requirement.hours} ชั่วโมง`
														: '—'}
											</span>
											{#if managementOpen && canManage && version.status === 'draft'}
												<LoadingButton
													variant="ghost"
													size="icon"
													loading={removingRequirementId === item.requirement.id}
													loadingLabel=""
													aria-label={`นำ ${item.catalog.name} ออกจากแผน`}
													onclick={() => removeRequirement(program, item)}
												>
													<Trash2 class="size-4 text-destructive" />
												</LoadingButton>
											{/if}
										</div>
									</div>
								{/each}
							</div>
						</div>
					{:else}
						<p class="px-4 py-5 text-sm text-muted-foreground">ยังไม่มีรายการในแผนนี้</p>
					{/each}
				</article>
			{:else}
				<div class="rounded-xl border border-dashed p-8 text-center text-sm text-muted-foreground">
					ยังไม่มีแผนการเรียนในรุ่นนี้
				</div>
			{/each}
		</div>

		{#if canManage && version.status === 'draft' && managementOpen}
			<aside class="space-y-4">
				<form class="space-y-3 rounded-xl border bg-muted/20 p-4" onsubmit={createProgram}>
					<h3 class="font-medium">เพิ่มแผนการเรียน</h3>
					<div class="space-y-1.5">
						<Label for={`program-code-${version.id}`}>รหัสแผน</Label>
						<Input id={`program-code-${version.id}`} bind:value={programDraft.code} required />
					</div>
					<div class="space-y-1.5">
						<Label for={`program-name-${version.id}`}>ชื่อแผน</Label>
						<Input id={`program-name-${version.id}`} bind:value={programDraft.nameTh} required />
					</div>
					<div class="space-y-1.5">
						<Label for={`program-name-en-${version.id}`}>ชื่อภาษาอังกฤษ (ถ้ามี)</Label>
						<Input id={`program-name-en-${version.id}`} bind:value={programDraft.nameEn} />
					</div>
					<label class="flex items-center gap-2 text-sm">
						<Checkbox bind:checked={programDraft.isDefault} /> ใช้เป็นแผนเริ่มต้น
					</label>
					<LoadingButton
						type="submit"
						variant="outline"
						class="w-full"
						loading={createProgramBusy}
						loadingLabel="กำลังเพิ่ม"
						disabled={!programDraft.code.trim() || !programDraft.nameTh.trim()}
					>
						<Plus class="size-4" /> เพิ่มแผน
					</LoadingButton>
				</form>

				{#if programs.length === 0}
					<AcademicPrerequisiteNotice prerequisite={noPrograms} />
				{:else if gradeLevels.length === 0}
					<AcademicPrerequisiteNotice prerequisite={noGradeLevels} />
				{:else if (managementOptions?.catalogVersions.length ?? 0) === 0}
					<AcademicPrerequisiteNotice prerequisite={noCatalogVersions} />
				{:else}
					<form class="space-y-3 rounded-xl border bg-muted/20 p-4" onsubmit={addRequirement}>
						<h3 class="font-medium">เพิ่มรายการในแผน</h3>
						<label class="space-y-1.5 text-sm">
							<span class="font-medium">แผนการเรียน</span>
							<Select.Root
								type="single"
								bind:value={requirementProgramId}
								onValueChange={() => (requirementProgramError = '')}
							>
								<Select.Trigger
									bind:ref={requirementProgramTrigger}
									class="w-full"
									aria-invalid={requirementProgramError ? true : undefined}
									aria-describedby={requirementProgramError
										? `requirement-program-error-${version.id}`
										: undefined}
								>
									{programs.find((program) => program.id === requirementProgramId)?.nameTh ??
										'เลือกแผนการเรียน'}
								</Select.Trigger>
								<Select.Content>
									{#each programs as program (program.id)}
										<Select.Item value={program.id}>{program.code} · {program.nameTh}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
							{#if requirementProgramError}
								<span
									id={`requirement-program-error-${version.id}`}
									role="alert"
									class="block text-xs text-destructive"
								>
									{requirementProgramError}
								</span>
							{/if}
						</label>
						<div class="grid grid-cols-2 gap-3">
							<label class="space-y-1.5 text-sm">
								<span class="font-medium">ประเภท</span>
								<Select.Root
									type="single"
									bind:value={requirementDraft.resourceKind}
									onValueChange={() => (requirementDraft.catalogVersionId = '')}
								>
									<Select.Trigger class="w-full">
										{requirementDraft.resourceKind === 'course' ? 'รายวิชา' : 'กิจกรรม'}
									</Select.Trigger>
									<Select.Content>
										<Select.Item value="course">รายวิชา</Select.Item>
										<Select.Item value="activity">กิจกรรม</Select.Item>
									</Select.Content>
								</Select.Root>
							</label>
							<label class="space-y-1.5 text-sm">
								<span class="font-medium">ข้อกำหนด</span>
								<Select.Root type="single" bind:value={requirementDraft.requirementKind}>
									<Select.Trigger class="w-full">
										{requirementKindLabels[requirementDraft.requirementKind]}
									</Select.Trigger>
									<Select.Content>
										<Select.Item value="required">บังคับ</Select.Item>
										<Select.Item value="elective">เลือก</Select.Item>
										<Select.Item value="optional">เพิ่มเติม</Select.Item>
									</Select.Content>
								</Select.Root>
							</label>
						</div>
						<label class="space-y-1.5 text-sm">
							<span class="font-medium">รายวิชาหรือกิจกรรม</span>
							<Select.Root type="single" bind:value={requirementDraft.catalogVersionId}>
								<Select.Trigger class="w-full"
									>{catalogLabel(requirementDraft.catalogVersionId)}</Select.Trigger
								>
								<Select.Content>
									{#each catalogVersions as option (option.id)}
										<Select.Item value={option.id}>
											{option.code} · {option.name} · รุ่น {option.versionNo}
										</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</label>
						<label class="space-y-1.5 text-sm">
							<span class="font-medium">ระดับชั้น</span>
							<Select.Root type="single" bind:value={requirementDraft.gradeLevelId}>
								<Select.Trigger class="w-full">
									{gradeLevels.find((grade) => grade.id === requirementDraft.gradeLevelId)?.name ??
										'เลือกระดับชั้น'}
								</Select.Trigger>
								<Select.Content>
									{#each gradeLevels as grade (grade.id)}
										<Select.Item value={grade.id}>{grade.name}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</label>
						<label class="space-y-1.5 text-sm">
							<span class="font-medium">ตำแหน่งภาคเรียนที่แนะนำ</span>
							<Select.Root type="single" bind:value={requirementDraft.recommendedTermCode}>
								<Select.Trigger class="w-full">
									{requirementDraft.recommendedTermCode === unspecifiedTerm
										? 'ไม่ระบุ'
										: (termLabels[requirementDraft.recommendedTermCode] ?? 'ไม่ระบุ')}
								</Select.Trigger>
								<Select.Content>
									<Select.Item value={unspecifiedTerm}>ไม่ระบุ</Select.Item>
									<Select.Item value="1">ภาคเรียนที่ 1</Select.Item>
									<Select.Item value="2">ภาคเรียนที่ 2</Select.Item>
									<Select.Item value="summer">ภาคฤดูร้อน</Select.Item>
								</Select.Content>
							</Select.Root>
						</label>
						<div class="grid grid-cols-2 gap-3">
							<div class="space-y-1.5">
								<Label for={`requirement-credit-${version.id}`}>หน่วยกิต</Label>
								<Input
									id={`requirement-credit-${version.id}`}
									bind:value={requirementDraft.credit}
								/>
							</div>
							<div class="space-y-1.5">
								<Label for={`requirement-hours-${version.id}`}>ชั่วโมง</Label>
								<Input id={`requirement-hours-${version.id}`} bind:value={requirementDraft.hours} />
							</div>
						</div>
						<LoadingButton
							type="submit"
							variant="outline"
							class="w-full"
							loading={requirementBusy}
							loadingLabel="กำลังเพิ่ม"
							disabled={!requirementDraft.catalogVersionId || !requirementDraft.gradeLevelId}
						>
							<Plus class="size-4" /> เพิ่มในแผน
						</LoadingButton>
					</form>
				{/if}
			</aside>
		{:else if version.status === 'published'}
			<aside class="rounded-xl border bg-muted/20 p-4 text-sm text-muted-foreground">
				รุ่นที่เผยแพร่แล้วเป็นข้อมูลอ้างอิงและแก้ไขไม่ได้
				หากต้องปรับโครงสร้างให้สร้างแบบร่างรุ่นใหม่
			</aside>
		{/if}
	</div>
	{#if errorMessage}
		<p role="alert" class="border-t px-5 py-3 text-sm text-destructive">{errorMessage}</p>
	{/if}
</section>
