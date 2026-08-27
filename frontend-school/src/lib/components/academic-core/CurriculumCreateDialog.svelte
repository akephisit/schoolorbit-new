<script lang="ts">
	import {
		createCurriculum,
		getCurriculumCreateOptions,
		type CurriculumCreateOptions,
		type CurriculumOverviewItem
	} from '$lib/api/academic-core';
	import { LoadingButton } from '$lib/components/app-state';
	import AcademicPrerequisiteNotice from '$lib/components/academic-workflow/AcademicPrerequisiteNotice.svelte';
	import type { AcademicPrerequisite } from '$lib/components/academic-workflow/prerequisite';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Plus } from 'lucide-svelte';
	import GradeLevelMultiSelect from './GradeLevelMultiSelect.svelte';

	let { onCreated }: { onCreated: (item: CurriculumOverviewItem) => void } = $props();

	let open = $state(false);
	let options = $state.raw<CurriculumCreateOptions | null>(null);
	let optionsLoading = $state(false);
	let saving = $state(false);
	let errorMessage = $state('');
	let draft = $state({
		code: '',
		nameTh: '',
		nameEn: '',
		gradeLevelIds: [] as string[]
	});

	const noGradeLevels: AcademicPrerequisite = {
		key: 'curriculum-grade-levels',
		status: 'missing',
		title: 'ยังไม่มีระดับชั้นให้เลือก',
		description: 'กรุณาติดต่อผู้ดูแลระบบให้ตั้งค่าระดับชั้นก่อนสร้างหลักสูตร'
	};

	async function showDialog() {
		open = true;
		if (options || optionsLoading) return;
		optionsLoading = true;
		errorMessage = '';
		try {
			options = await getCurriculumCreateOptions();
		} catch (error) {
			errorMessage =
				error instanceof Error ? error.message : 'โหลดตัวเลือกสำหรับสร้างหลักสูตรไม่สำเร็จ';
		} finally {
			optionsLoading = false;
		}
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (!options || draft.gradeLevelIds.length === 0) return;
		saving = true;
		errorMessage = '';
		try {
			const curriculum = await createCurriculum({
				code: draft.code.trim(),
				nameTh: draft.nameTh.trim(),
				nameEn: draft.nameEn.trim() || null,
				description: null,
				gradeLevelIds: draft.gradeLevelIds,
				owningOrganizationUnitId: null
			});
			onCreated({
				curriculum,
				displayVersion: null,
				displayState: 'unpublished',
				gradeLevels: options.gradeLevels.filter((level) => draft.gradeLevelIds.includes(level.id)),
				startAcademicYearName: null,
				endAcademicYearName: null,
				studyProgramCount: 0,
				draftCount: 0
			});
			draft = { code: '', nameTh: '', nameEn: '', gradeLevelIds: [] };
			open = false;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างหลักสูตรไม่สำเร็จ';
		} finally {
			saving = false;
		}
	}
</script>

<Button onclick={showDialog}><Plus class="size-4" /> เพิ่มหลักสูตร</Button>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-xl">
		<Dialog.Header>
			<Dialog.Title>เพิ่มหลักสูตร</Dialog.Title>
			<Dialog.Description>
				สร้างตัวตนหลักสูตรก่อน แล้วจึงเพิ่มรุ่นและแผนการเรียนในหน้ารายละเอียด
			</Dialog.Description>
		</Dialog.Header>
		{#if optionsLoading}
			<div class="space-y-3 py-3" aria-label="กำลังโหลดตัวเลือก">
				<div class="h-10 animate-pulse rounded-md bg-muted"></div>
				<div class="h-10 animate-pulse rounded-md bg-muted"></div>
				<div class="h-10 animate-pulse rounded-md bg-muted"></div>
			</div>
		{:else if options}
			{#if options.gradeLevels.length === 0}
				<AcademicPrerequisiteNotice prerequisite={noGradeLevels} class="my-2" />
			{:else}
				<form class="space-y-4 py-2" onsubmit={submit}>
					<div class="grid gap-4 sm:grid-cols-2">
						<div class="space-y-2">
							<Label for="curriculum-code">รหัสหลักสูตร</Label>
							<Input id="curriculum-code" bind:value={draft.code} required />
						</div>
						<div class="space-y-2">
							<Label for="curriculum-name-th">ชื่อหลักสูตรภาษาไทย</Label>
							<Input id="curriculum-name-th" bind:value={draft.nameTh} required />
						</div>
					</div>
					<div class="space-y-2">
						<Label for="curriculum-name-en">ชื่อภาษาอังกฤษ (ถ้ามี)</Label>
						<Input id="curriculum-name-en" bind:value={draft.nameEn} />
					</div>
					<div class="space-y-2">
						<Label>ระดับชั้นที่หลักสูตรครอบคลุม</Label>
						<GradeLevelMultiSelect
							bind:value={draft.gradeLevelIds}
							options={options.gradeLevels}
							ariaLabel="เลือกระดับชั้นของหลักสูตร"
						/>
					</div>
					{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
					<Dialog.Footer>
						<Button type="button" variant="outline" onclick={() => (open = false)}>ยกเลิก</Button>
						<LoadingButton
							type="submit"
							loading={saving}
							loadingLabel="กำลังสร้าง"
							disabled={!draft.code.trim() ||
								!draft.nameTh.trim() ||
								draft.gradeLevelIds.length === 0}
						>
							สร้างหลักสูตร
						</LoadingButton>
					</Dialog.Footer>
				</form>
			{/if}
		{:else}
			<div class="space-y-3 py-4">
				<p role="alert" class="text-sm text-destructive">{errorMessage}</p>
				<Button type="button" variant="outline" onclick={showDialog}>ลองโหลดอีกครั้ง</Button>
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>
