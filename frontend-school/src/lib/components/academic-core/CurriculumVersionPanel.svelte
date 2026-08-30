<script lang="ts">
	import type {
		CloneCurriculumVersionRequest,
		CreateCurriculumVersionRequest,
		Curriculum,
		CurriculumCreateOptions,
		CurriculumVersionView
	} from '$lib/api/academic-core';
	import { LoadingButton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { BookCopy, GitBranchPlus } from 'lucide-svelte';

	let {
		curriculum,
		versions,
		selectedVersion,
		canManage,
		onSelectVersion,
		onRequestCreateOptions,
		onCreateVersion,
		onCloneVersion
	}: {
		curriculum: Curriculum;
		versions: CurriculumVersionView[];
		selectedVersion: CurriculumVersionView | null;
		canManage: boolean;
		onSelectVersion: (version: CurriculumVersionView) => Promise<void>;
		onRequestCreateOptions: () => Promise<CurriculumCreateOptions | null>;
		onCreateVersion: (draft: CreateCurriculumVersionRequest) => Promise<void>;
		onCloneVersion: (
			sourceVersionId: string,
			draft: CloneCurriculumVersionRequest
		) => Promise<void>;
	} = $props();

	const noEndYear = 'no-end';
	let createOpen = $state(false);
	let optionsLoading = $state(false);
	let saving = $state(false);
	let errorMessage = $state('');
	let createYears = $state.raw<CurriculumCreateOptions['academicYears']>([]);
	let cloneSource = $state.raw<CurriculumVersionView | null>(null);
	let draft = $state({
		versionName: '',
		startAcademicYearId: '',
		endAcademicYearId: noEndYear,
		description: ''
	});

	function versionRange(view: CurriculumVersionView) {
		const start = view.startAcademicYearName;
		const end = view.endAcademicYearName;
		return end ? `${start}–${end}` : `ตั้งแต่ ${start}`;
	}

	function versionStatusLabel(view: CurriculumVersionView) {
		return view.version.status === 'published'
			? 'เผยแพร่แล้ว'
			: view.version.status === 'archived'
				? 'เก็บถาวร'
				: 'แบบร่าง';
	}

	let selectedStartYear = $derived(
		createYears.find((year) => year.id === draft.startAcademicYearId) ?? null
	);
	let availableYears = $derived.by(() => {
		if (!cloneSource) return createYears;
		const sourceYear = createYears.find(
			(year) => year.id === cloneSource?.version.startAcademicYearId
		);
		return sourceYear ? createYears.filter((year) => year.year > sourceYear.year) : [];
	});
	let selectedEndYear = $derived(
		createYears.find((year) => year.id === draft.endAcademicYearId) ?? null
	);
	let endYearIsValid = $derived(
		!selectedEndYear || !selectedStartYear || selectedEndYear.year >= selectedStartYear.year
	);

	async function showCreateDialog() {
		cloneSource = selectedVersion?.version.status === 'published' ? selectedVersion : null;
		createOpen = true;
		errorMessage = '';
		optionsLoading = true;
		try {
			const options = await onRequestCreateOptions();
			createYears = options?.academicYears ?? [];
			const sourceYear = cloneSource
				? createYears.find((year) => year.id === cloneSource?.version.startAcademicYearId)
				: null;
			const eligibleYears = sourceYear
				? createYears.filter((year) => year.year > sourceYear.year)
				: createYears;
			draft.startAcademicYearId = eligibleYears[0]?.id ?? '';
			draft.endAcademicYearId = noEndYear;
			draft.versionName = cloneSource ? `${cloneSource.version.versionName} รุ่นใหม่` : '';
			draft.description = '';
		} catch (error) {
			errorMessage =
				error instanceof Error ? error.message : 'โหลดปีการศึกษาสำหรับสร้างรุ่นไม่สำเร็จ';
		} finally {
			optionsLoading = false;
		}
	}

	async function createVersion(event: SubmitEvent) {
		event.preventDefault();
		if (!endYearIsValid) return;
		saving = true;
		errorMessage = '';
		try {
			const versionDraft: CreateCurriculumVersionRequest = {
				versionName: draft.versionName.trim(),
				startAcademicYearId: draft.startAcademicYearId,
				endAcademicYearId: draft.endAcademicYearId === noEndYear ? null : draft.endAcademicYearId,
				description: draft.description.trim() || null
			};
			if (cloneSource) {
				await onCloneVersion(cloneSource.version.id, {
					...versionDraft,
					sourceRowVersion: cloneSource.version.rowVersion
				});
			} else {
				await onCreateVersion(versionDraft);
			}
			draft = {
				versionName: '',
				startAcademicYearId: availableYears[0]?.id ?? '',
				endAcademicYearId: noEndYear,
				description: ''
			};
			createOpen = false;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างรุ่นหลักสูตรไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}
</script>

<section class="overflow-hidden rounded-2xl border bg-card shadow-sm">
	<header
		class="flex flex-col gap-4 border-b bg-muted/25 p-5 lg:flex-row lg:items-start lg:justify-between"
	>
		<div class="flex min-w-0 items-start gap-3">
			<div class="rounded-xl bg-primary/10 p-2.5 text-primary"><BookCopy class="size-5" /></div>
			<div class="min-w-0">
				<p class="font-mono text-sm font-semibold text-primary">{curriculum.code}</p>
				<h1 class="mt-1 text-xl font-semibold tracking-tight">{curriculum.nameTh}</h1>
				{#if curriculum.nameEn}
					<p class="mt-1 text-sm text-muted-foreground">{curriculum.nameEn}</p>
				{/if}
			</div>
		</div>
		{#if canManage}
			<Button variant="outline" onclick={showCreateDialog}>
				<GitBranchPlus class="size-4" />
				{selectedVersion?.version.status === 'published'
					? 'สร้างหลักสูตรรุ่นใหม่แบบร่าง'
					: 'เพิ่มรุ่นหลักสูตร'}
			</Button>
		{/if}
	</header>

	<div class="p-4 sm:p-5">
		<div class="mb-3 flex items-center justify-between gap-3">
			<div>
				<h2 class="font-medium">ประวัติรุ่นหลักสูตร</h2>
				<p class="text-xs text-muted-foreground">เลือกรุ่นเพื่อดูแผนการเรียนและรายการในแผน</p>
			</div>
			<Badge variant="secondary">{versions.length} รุ่น</Badge>
		</div>
		{#if versions.length === 0}
			<div class="rounded-xl border border-dashed p-6 text-center text-sm text-muted-foreground">
				ยังไม่มีรุ่นหลักสูตร
			</div>
		{:else}
			<div class="flex gap-2 overflow-x-auto pb-1">
				{#each versions as version (version.version.id)}
					<Button
						variant={selectedVersion?.version.id === version.version.id ? 'default' : 'outline'}
						class="h-auto min-w-44 flex-col items-start gap-1 px-3 py-2 text-start"
						onclick={() => onSelectVersion(version)}
					>
						<span class="w-full truncate font-medium">{version.version.versionName}</span>
						<span class="text-xs opacity-80">{versionRange(version)}</span>
						<span class="text-xs opacity-80">{versionStatusLabel(version)}</span>
					</Button>
				{/each}
			</div>
		{/if}
	</div>
</section>

<Dialog.Root bind:open={createOpen}>
	<Dialog.Content class="sm:max-w-xl">
		<Dialog.Header>
			<Dialog.Title>
				{cloneSource ? 'สร้างหลักสูตรรุ่นใหม่แบบร่าง' : 'สร้างรุ่นหลักสูตรแบบร่าง'}
			</Dialog.Title>
			<Dialog.Description>
				{#if cloneSource}
					ระบบจะคัดลอกภาคเรียน แผนการเรียน รายวิชา และกิจกรรมทั้งหมดไปเป็นแบบร่างใหม่
					ต้นฉบับที่เผยแพร่จะไม่เปลี่ยน
				{:else}
					กำหนดช่วงปีการศึกษาที่ตั้งใจจะใช้ รุ่นใหม่จะยังแก้ไขได้จนกว่าจะเผยแพร่
				{/if}
			</Dialog.Description>
		</Dialog.Header>
		{#if optionsLoading}
			<div class="space-y-3 py-3" aria-label="กำลังโหลดปีการศึกษา">
				<div class="h-10 animate-pulse rounded-md bg-muted"></div>
				<div class="h-10 animate-pulse rounded-md bg-muted"></div>
			</div>
		{:else if availableYears.length === 0}
			<div class="rounded-xl border border-dashed p-5 text-sm text-muted-foreground">
				{cloneSource
					? 'ยังไม่มีปีการศึกษาถัดจากรุ่นต้นทาง กรุณาสร้างปีการศึกษาก่อน'
					: 'ยังไม่มีปีการศึกษาสำหรับสร้างรุ่นหลักสูตร'}
			</div>
		{:else}
			<form class="space-y-4 py-2" onsubmit={createVersion}>
				<div class="space-y-2">
					<Label for="curriculum-version-name">ชื่อรุ่น</Label>
					<Input
						id="curriculum-version-name"
						bind:value={draft.versionName}
						placeholder="เช่น หลักสูตรสถานศึกษา 2569"
						required
					/>
				</div>
				<div class="grid gap-4 sm:grid-cols-2">
					<label class="space-y-2 text-sm">
						<span class="font-medium">เริ่มใช้ในปีการศึกษา</span>
						<Select.Root type="single" bind:value={draft.startAcademicYearId}>
							<Select.Trigger class="w-full">
								{availableYears.find((year) => year.id === draft.startAcademicYearId)?.name ??
									'เลือกปีเริ่มใช้'}
							</Select.Trigger>
							<Select.Content>
								{#each availableYears as year (year.id)}
									<Select.Item value={year.id}>{year.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</label>
					<label class="space-y-2 text-sm">
						<span class="font-medium">สิ้นสุดในปีการศึกษา</span>
						<Select.Root type="single" bind:value={draft.endAcademicYearId}>
							<Select.Trigger class="w-full" aria-invalid={!endYearIsValid}>
								{draft.endAcademicYearId === noEndYear
									? 'ไม่กำหนด'
									: (createYears.find((year) => year.id === draft.endAcademicYearId)?.name ??
										'เลือกปีสิ้นสุด')}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value={noEndYear}>ไม่กำหนด</Select.Item>
								{#each createYears as year (year.id)}
									<Select.Item value={year.id}>{year.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
						{#if !endYearIsValid}
							<span class="block text-xs text-destructive">ปีสิ้นสุดต้องไม่ก่อนปีเริ่มใช้</span>
						{/if}
					</label>
				</div>
				<div class="space-y-2">
					<Label for="curriculum-version-description">คำอธิบาย (ถ้ามี)</Label>
					<Input id="curriculum-version-description" bind:value={draft.description} />
				</div>
				{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
				<Dialog.Footer>
					<Button type="button" variant="outline" onclick={() => (createOpen = false)}
						>ยกเลิก</Button
					>
					<LoadingButton
						type="submit"
						loading={saving}
						loadingLabel="กำลังสร้าง"
						disabled={!draft.versionName.trim() || !draft.startAcademicYearId || !endYearIsValid}
					>
						สร้างแบบร่าง
					</LoadingButton>
				</Dialog.Footer>
			</form>
		{/if}
		{#if errorMessage && (optionsLoading || availableYears.length === 0)}
			<p role="alert" class="text-sm text-destructive">{errorMessage}</p>
		{/if}
	</Dialog.Content>
</Dialog.Root>
