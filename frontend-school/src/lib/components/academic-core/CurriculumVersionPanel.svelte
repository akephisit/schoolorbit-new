<script lang="ts">
	import type {
		CloneCurriculumVersionRequest,
		CreateCurriculumVersionRequest,
		Curriculum,
		CurriculumVersionView
	} from '#lib/api/academic-core.js';
	import { curriculumEditionLabel } from '#lib/academic-core/curriculum-presentation.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { BookCopy, GitBranchPlus } from '@lucide/svelte';

	let {
		curriculum,
		versions,
		selectedVersion,
		canManage,
		onSelectVersion,
		onCreateVersion,
		onCloneVersion
	}: {
		curriculum: Curriculum;
		versions: CurriculumVersionView[];
		selectedVersion: CurriculumVersionView | null;
		canManage: boolean;
		onSelectVersion: (version: CurriculumVersionView) => Promise<void>;
		onCreateVersion: (draft: CreateCurriculumVersionRequest) => Promise<void>;
		onCloneVersion: (
			sourceVersionId: string,
			draft: CloneCurriculumVersionRequest
		) => Promise<void>;
	} = $props();

	let createOpen = $state(false);
	let saving = $state(false);
	let errorMessage = $state('');
	let cloneSource = $state.raw<CurriculumVersionView | null>(null);
	let draft = $state<{
		revisionYear: number | undefined;
		versionName: string;
		description: string;
	}>({ revisionYear: new Date().getFullYear() + 543, versionName: '', description: '' });
	const validRevisionYear = $derived(
		draft.revisionYear !== undefined &&
			Number.isInteger(draft.revisionYear) &&
			draft.revisionYear >= 2400 &&
			draft.revisionYear <= 2999
	);

	function versionStatusLabel(view: CurriculumVersionView) {
		return view.version.status === 'published'
			? 'เผยแพร่แล้ว'
			: view.version.status === 'archived'
				? 'เก็บถาวร'
				: 'แบบร่าง';
	}

	function showCreateDialog() {
		cloneSource = selectedVersion?.version.status === 'published' ? selectedVersion : null;
		draft = { revisionYear: new Date().getFullYear() + 543, versionName: '', description: '' };
		errorMessage = '';
		createOpen = true;
	}

	async function createVersion(event: SubmitEvent) {
		event.preventDefault();
		if (!validRevisionYear || draft.revisionYear === undefined) return;
		saving = true;
		errorMessage = '';
		try {
			const versionDraft: CreateCurriculumVersionRequest = {
				versionName: draft.versionName.trim() || `ฉบับปรับปรุง พุทธศักราช ${draft.revisionYear}`,
				revisionYear: draft.revisionYear,
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
			createOpen = false;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างฉบับหลักสูตรไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}
</script>

<section class="overflow-hidden rounded-2xl border bg-card">
	<header
		class="flex flex-col gap-4 border-b bg-muted/25 p-4 sm:p-5 lg:flex-row lg:items-start lg:justify-between"
	>
		<div class="flex min-w-0 items-start gap-3">
			<div class="rounded-xl bg-primary/10 p-2.5 text-primary"><BookCopy class="size-5" /></div>
			<div class="min-w-0">
				<p class="text-sm text-muted-foreground">หลักสูตรสถานศึกษา</p>
				<h2 class="mt-1 text-xl font-semibold tracking-tight">{curriculum.nameTh}</h2>
				<p class="mt-1 text-sm text-muted-foreground">
					ฉบับหลักสูตร → แผนการเรียน → ระดับชั้น → ภาคเรียน
				</p>
			</div>
		</div>
		{#if canManage}
			<Button variant="outline" onclick={showCreateDialog}
				><GitBranchPlus class="size-4" />{selectedVersion?.version.status === 'published'
					? 'สร้างฉบับปรับปรุงจากฉบับนี้'
					: 'เพิ่มฉบับหลักสูตร'}</Button
			>
		{/if}
	</header>
	<div class="p-4 sm:p-5">
		<div class="mb-3 flex items-center justify-between gap-3">
			<div>
				<h3 class="font-medium">ฉบับหลักสูตร</h3>
				<p class="text-xs text-muted-foreground">
					เลือกฉบับเพื่อดูแผนการเรียน ปีปรับปรุงใช้ระบุฉบับและไม่จำกัดปีที่นำไปใช้
				</p>
			</div>
			<Badge variant="secondary">{versions.length} ฉบับ</Badge>
		</div>
		{#if versions.length === 0}
			<p class="rounded-xl border border-dashed p-6 text-center text-sm text-muted-foreground">
				ยังไม่มีฉบับหลักสูตร เพิ่มฉบับแบบร่างเพื่อเริ่มจัดแผนการเรียน
			</p>
		{:else}
			<div class="flex flex-wrap gap-2">
				{#each versions as version (version.version.id)}
					<Button
						variant={selectedVersion?.version.id === version.version.id ? 'default' : 'outline'}
						class="h-auto min-w-44 flex-col items-start gap-1 px-3 py-2 text-start"
						onclick={() => onSelectVersion(version)}
					>
						<span class="whitespace-normal font-medium"
							>{curriculumEditionLabel(version.version)}</span
						>
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
			<Dialog.Title
				>{cloneSource ? 'สร้างฉบับปรับปรุงแบบร่าง' : 'เพิ่มฉบับหลักสูตรแบบร่าง'}</Dialog.Title
			>
			<Dialog.Description
				>{#if cloneSource}ระบบจะคัดลอกภาคเรียน แผนการเรียน รายวิชา
					และกิจกรรมทั้งหมดไปเป็นแบบร่างใหม่ ต้นฉบับที่เผยแพร่จะไม่เปลี่ยน{:else}ระบุปีปรับปรุงหลักสูตร
					แล้วจัดแผนการเรียนและรายวิชาในฉบับนี้ ภาคเรียนเริ่มต้น 2 ภาคเรียนสามารถปรับได้{/if}</Dialog.Description
			>
		</Dialog.Header>
		<form class="space-y-4" onsubmit={createVersion}>
			<div class="space-y-2">
				<Label for="curriculum-revision-year">ปีปรับปรุงหลักสูตร (พุทธศักราช)</Label><Input
					id="curriculum-revision-year"
					type="number"
					min="2400"
					max="2999"
					step="1"
					bind:value={draft.revisionYear}
					required
				/>
				<p class="text-xs text-muted-foreground">
					เช่น 2569 โรงเรียนเลือกใช้ฉบับนี้ตอนจัดห้องได้ในปีการศึกษาอื่นด้วย
				</p>
			</div>
			<div class="space-y-2">
				<Label for="curriculum-version-name">ชื่อฉบับ (ถ้ามี)</Label><Input
					id="curriculum-version-name"
					bind:value={draft.versionName}
					placeholder={`ฉบับปรับปรุง พุทธศักราช ${draft.revisionYear ?? '2569'}`}
				/>
			</div>
			<div class="space-y-2">
				<Label for="curriculum-version-description">คำอธิบาย (ถ้ามี)</Label><Input
					id="curriculum-version-description"
					bind:value={draft.description}
				/>
			</div>
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
			<Dialog.Footer
				><Button type="button" variant="outline" onclick={() => (createOpen = false)}>ยกเลิก</Button
				><LoadingButton
					type="submit"
					loading={saving}
					loadingLabel="กำลังสร้าง"
					disabled={!validRevisionYear}>สร้างแบบร่าง</LoadingButton
				></Dialog.Footer
			>
		</form>
	</Dialog.Content>
</Dialog.Root>
