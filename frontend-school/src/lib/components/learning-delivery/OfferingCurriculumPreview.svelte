<script lang="ts">
	import {
		applyLearningOfferingsFromCurriculum,
		previewLearningOfferingsFromCurriculum,
		type CurriculumOfferingPreview,
		type DeliveryManagementOptions
	} from '$lib/api/learning-delivery';
	import { LoadingButton } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as Command from '$lib/components/ui/command';
	import { Label } from '$lib/components/ui/label';
	import * as Popover from '$lib/components/ui/popover';
	import { ChevronsUpDown, Eye, WandSparkles } from 'lucide-svelte';
	import DeliveryOptionCombobox from './DeliveryOptionCombobox.svelte';

	let {
		academicTermId,
		options,
		onApplied
	}: {
		academicTermId: string;
		options: DeliveryManagementOptions;
		onApplied: () => Promise<void> | void;
	} = $props();

	let programPickerOpen = $state(false);
	let programSearch = $state('');
	let studyProgramIds = $state<string[]>([]);
	let owningOrganizationUnitId = $state('');
	let preview = $state.raw<CurriculumOfferingPreview | null>(null);
	let busy = $state(false);
	let errorMessage = $state('');
	let filteredPrograms = $derived.by(() => {
		const query = programSearch.trim().toLocaleLowerCase('th-TH');
		if (!query) return options.studyPrograms;
		return options.studyPrograms.filter((program) =>
			`${program.curriculumName} ${program.code} ${program.name}`
				.toLocaleLowerCase('th-TH')
				.includes(query)
		);
	});
	let selectedProgramLabel = $derived.by(() => {
		if (studyProgramIds.length === 0) return 'เลือกแผนการเรียน';
		if (studyProgramIds.length === 1)
			return (
				options.studyPrograms.find((program) => program.id === studyProgramIds[0])?.name ??
				'เลือกแล้ว 1 แผน'
			);
		return `เลือกแล้ว ${studyProgramIds.length} แผน`;
	});

	function toggleProgram(id: string) {
		studyProgramIds = studyProgramIds.includes(id)
			? studyProgramIds.filter((programId) => programId !== id)
			: [...studyProgramIds, id];
		preview = null;
	}

	function gradeLabel(gradeLevelId: string) {
		const grade = options.gradeLevels.find((item) => item.id === gradeLevelId);
		return grade?.short_name ?? grade?.name ?? 'ไม่พบชื่อระดับชั้น';
	}

	function programLabel(programId: string) {
		return (
			options.studyPrograms.find((item) => item.id === programId)?.name ?? 'ไม่พบชื่อแผนการเรียน'
		);
	}

	async function buildPreview() {
		if (studyProgramIds.length === 0) return;
		busy = true;
		errorMessage = '';
		try {
			preview = await previewLearningOfferingsFromCurriculum({ academicTermId, studyProgramIds });
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างตัวอย่างจากหลักสูตรไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}

	async function applyPreview() {
		if (!preview || !owningOrganizationUnitId) return;
		busy = true;
		errorMessage = '';
		try {
			await applyLearningOfferingsFromCurriculum({
				academicTermId,
				studyProgramIds,
				owningOrganizationUnitId,
				sourceHash: preview.sourceHash,
				idempotencyKey: crypto.randomUUID()
			});
			await onApplied();
			preview = null;
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'นำรายการจากหลักสูตรมาใช้ไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}
</script>

<div class="space-y-4">
	<div class="rounded-xl border border-primary/20 bg-primary/[0.035] p-4">
		<div class="flex items-start gap-3">
			<div class="rounded-lg bg-primary/10 p-2 text-primary"><WandSparkles class="size-4" /></div>
			<div>
				<h3 class="font-medium">นำข้อกำหนดจากหลักสูตรมาเปิดสอน</h3>
				<p class="mt-1 text-sm text-muted-foreground">
					ระบบจะแสดงรายการที่จะสร้าง คงเดิม หรือขัดแย้งก่อนบันทึกจริง
				</p>
			</div>
		</div>
	</div>

	<div class="grid gap-4 sm:grid-cols-2">
		<div class="space-y-2">
			<Label>แผนการเรียน</Label>
			<Popover.Root bind:open={programPickerOpen}>
				<Popover.Trigger>
					{#snippet child({ props })}
						<Button
							type="button"
							variant="outline"
							role="combobox"
							aria-label="เลือกแผนการเรียนจากหลักสูตร"
							aria-expanded={programPickerOpen}
							class="w-full justify-between font-normal"
							{...props}
						>
							<span class="truncate">{selectedProgramLabel}</span><ChevronsUpDown
								class="size-4 opacity-50"
							/>
						</Button>
					{/snippet}
				</Popover.Trigger>
				<Popover.Content class="w-[--bits-popover-trigger-width] p-0" align="start">
					<Command.Root shouldFilter={false}>
						<Command.Input
							bind:value={programSearch}
							placeholder="ค้นหาหลักสูตรหรือแผนการเรียน..."
						/>
						<Command.List class="max-h-64">
							{#if filteredPrograms.length === 0}<Command.Empty>ไม่พบแผนการเรียน</Command.Empty
								>{:else}<Command.Group>
									{#each filteredPrograms as program (program.id)}
										<Command.Item
											value={`${program.curriculumName} ${program.code} ${program.name}`}
											onSelect={() => toggleProgram(program.id)}
										>
											<Checkbox
												checked={studyProgramIds.includes(program.id)}
												class="pointer-events-none"
												aria-label={`เลือก ${program.name}`}
											/>
											<div class="min-w-0">
												<p class="truncate">{program.name}</p>
												<p class="truncate text-xs text-muted-foreground">
													{program.curriculumName} · {program.code}
												</p>
											</div>
										</Command.Item>
									{/each}
								</Command.Group>{/if}
						</Command.List>
					</Command.Root>
				</Popover.Content>
			</Popover.Root>
		</div>
		<div class="space-y-2">
			<Label>หน่วยงานเจ้าของรายการเปิดสอน</Label>
			<DeliveryOptionCombobox
				bind:value={owningOrganizationUnitId}
				options={options.organizationUnits.map((unit) => ({
					id: unit.id,
					label: unit.name,
					description: unit.code
				}))}
				placeholder="เลือกหน่วยงานเจ้าของ"
				searchPlaceholder="ค้นหาชื่อหรือรหัสหน่วยงาน..."
			/>
		</div>
	</div>
	<div class="flex justify-end">
		<LoadingButton
			type="button"
			variant="outline"
			loading={busy}
			loadingLabel="กำลังตรวจ"
			disabled={studyProgramIds.length === 0}
			onclick={buildPreview}><Eye class="size-4" /> ตรวจรายการก่อน</LoadingButton
		>
	</div>

	{#if preview}
		<div class="overflow-hidden rounded-xl border">
			<div class="flex items-center justify-between gap-3 border-b bg-muted/30 px-4 py-3">
				<div>
					<h3 class="font-medium">ผลตรวจจากหลักสูตร</h3>
					<p class="text-xs text-muted-foreground">{preview.items.length} รายการ</p>
				</div>
				<Badge variant="secondary">ยังไม่บันทึก</Badge>
			</div>
			<div class="max-h-72 divide-y overflow-auto">
				{#each preview.items as item (item.requirementId)}
					<div
						class="grid gap-2 px-4 py-3 text-sm sm:grid-cols-[auto_minmax(0,1fr)_auto] sm:items-center"
					>
						<Badge
							variant={item.action === 'conflict'
								? 'destructive'
								: item.action === 'create'
									? 'default'
									: 'secondary'}
							>{item.action === 'create'
								? 'สร้างใหม่'
								: item.action === 'retain'
									? 'มีอยู่แล้ว'
									: 'ขัดแย้ง'}</Badge
						>
						<div class="min-w-0">
							<p class="font-medium">{item.code} · {item.name}</p>
							<p class="text-xs text-muted-foreground">
								{gradeLabel(item.gradeLevelId)} · {programLabel(
									item.studyProgramId
								)}{item.conflictReason ? ` · ${item.conflictReason}` : ''}
							</p>
						</div>
						<span class="text-xs tabular-nums text-muted-foreground"
							>{item.credit
								? `${item.credit} หน่วยกิต`
								: item.hours
									? `${item.hours} ชม.`
									: '—'}</span
						>
					</div>
				{/each}
			</div>
			<div class="flex justify-end border-t p-3">
				<LoadingButton
					type="button"
					loading={busy}
					loadingLabel="กำลังนำมาใช้"
					disabled={!owningOrganizationUnitId ||
						preview.items.some((item) => item.action === 'conflict')}
					onclick={applyPreview}
					><WandSparkles class="size-4" /> นำรายการที่ตรวจแล้วมาใช้</LoadingButton
				>
			</div>
		</div>
	{/if}
	{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
</div>
