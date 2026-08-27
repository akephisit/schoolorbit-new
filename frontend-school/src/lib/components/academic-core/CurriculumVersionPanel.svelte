<script lang="ts">
	import type {
		AcademicYear,
		CreateCurriculumVersionRequest,
		Curriculum,
		CurriculumManagementOptions,
		CurriculumVersion
	} from '$lib/api/academic-core';
	import { LoadingButton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { BookCopy, GitBranchPlus } from 'lucide-svelte';

	type AcademicYearChoice = Pick<AcademicYear, 'id' | 'name' | 'year' | 'status'>;

	let {
		curriculum,
		versions,
		selectedVersion,
		academicYears,
		canManage,
		onSelectVersion,
		onRequestManagementOptions,
		onCreateVersion
	}: {
		curriculum: Curriculum;
		versions: CurriculumVersion[];
		selectedVersion: CurriculumVersion | null;
		academicYears: AcademicYear[];
		canManage: boolean;
		onSelectVersion: (version: CurriculumVersion) => Promise<void>;
		onRequestManagementOptions: () => Promise<CurriculumManagementOptions | null>;
		onCreateVersion: (draft: CreateCurriculumVersionRequest) => Promise<void>;
	} = $props();

	const noEndYear = 'no-end';
	let createOpen = $state(false);
	let optionsLoading = $state(false);
	let saving = $state(false);
	let errorMessage = $state('');
	let createYears = $state.raw<AcademicYearChoice[]>([]);
	let draft = $state({
		versionName: '',
		startAcademicYearId: '',
		endAcademicYearId: noEndYear,
		description: ''
	});

	function yearLabel(yearId: string | null | undefined) {
		if (!yearId) return 'ไม่กำหนด';
		return academicYears.find((year) => year.id === yearId)?.name ?? 'ไม่พบปีการศึกษา';
	}

	function versionRange(version: CurriculumVersion) {
		const start = yearLabel(version.startAcademicYearId);
		const end = version.endAcademicYearId ? yearLabel(version.endAcademicYearId) : null;
		return end ? `${start}–${end}` : `ตั้งแต่ ${start}`;
	}

	function versionStatusLabel(version: CurriculumVersion) {
		return version.status === 'published'
			? 'เผยแพร่แล้ว'
			: version.status === 'archived'
				? 'เก็บถาวร'
				: 'แบบร่าง';
	}

	let selectedStartYear = $derived(
		createYears.find((year) => year.id === draft.startAcademicYearId) ?? null
	);
	let selectedEndYear = $derived(
		createYears.find((year) => year.id === draft.endAcademicYearId) ?? null
	);
	let endYearIsValid = $derived(
		!selectedEndYear || !selectedStartYear || selectedEndYear.year >= selectedStartYear.year
	);

	async function showCreateDialog() {
		createOpen = true;
		errorMessage = '';
		optionsLoading = true;
		try {
			const options = selectedVersion ? await onRequestManagementOptions() : null;
			createYears = options?.academicYears ?? academicYears;
			if (!draft.startAcademicYearId && createYears[0]) {
				draft.startAcademicYearId = createYears[0].id;
			}
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
			await onCreateVersion({
				versionName: draft.versionName.trim(),
				startAcademicYearId: draft.startAcademicYearId,
				endAcademicYearId: draft.endAcademicYearId === noEndYear ? null : draft.endAcademicYearId,
				description: draft.description.trim() || null
			});
			draft = {
				versionName: '',
				startAcademicYearId: createYears[0]?.id ?? '',
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
				{selectedVersion?.status === 'published' ? 'สร้างแบบร่างรุ่นใหม่' : 'เพิ่มรุ่นหลักสูตร'}
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
				{#each versions as version (version.id)}
					<Button
						variant={selectedVersion?.id === version.id ? 'default' : 'outline'}
						class="h-auto min-w-44 flex-col items-start gap-1 px-3 py-2 text-start"
						onclick={() => onSelectVersion(version)}
					>
						<span class="w-full truncate font-medium">{version.versionName}</span>
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
			<Dialog.Title>สร้างรุ่นหลักสูตรแบบร่าง</Dialog.Title>
			<Dialog.Description>
				กำหนดช่วงปีการศึกษาที่ตั้งใจจะใช้ รุ่นใหม่จะยังแก้ไขได้จนกว่าจะเผยแพร่
			</Dialog.Description>
		</Dialog.Header>
		{#if optionsLoading}
			<div class="space-y-3 py-3" aria-label="กำลังโหลดปีการศึกษา">
				<div class="h-10 animate-pulse rounded-md bg-muted"></div>
				<div class="h-10 animate-pulse rounded-md bg-muted"></div>
			</div>
		{:else if createYears.length === 0}
			<div class="rounded-xl border border-dashed p-5 text-sm text-muted-foreground">
				ยังไม่มีปีการศึกษาสำหรับสร้างรุ่นหลักสูตร
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
								{createYears.find((year) => year.id === draft.startAcademicYearId)?.name ??
									'เลือกปีเริ่มใช้'}
							</Select.Trigger>
							<Select.Content>
								{#each createYears as year (year.id)}
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
		{#if errorMessage && (optionsLoading || createYears.length === 0)}
			<p role="alert" class="text-sm text-destructive">{errorMessage}</p>
		{/if}
	</Dialog.Content>
</Dialog.Root>
