<script lang="ts">
	import { onMount } from 'svelte';
	import type {
		CurriculumManagementOptions,
		CurriculumStructureRequirementInput
	} from '#lib/api/academic-core.js';
	import { ApiClientError } from '#lib/api/client.js';
	import { isAbortError } from '#lib/async/latest-request.js';
	import {
		quickActionRequirements,
		stageCurriculumRequirements,
		type CurriculumQuickActionBaseline
	} from '#lib/academic-core/curriculum-quick-actions.js';
	import {
		selectableCatalogOption,
		hasExistingCatalogRequirement
	} from '#lib/academic-core/curriculum-catalog-selection.js';
	import { curriculumMetric } from '#lib/academic/curriculum-structure.js';
	import { formatEffectiveFrom } from '#lib/academic-core/catalog-presentation.js';
	import { PageState, PageSkeleton, LoadingButton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Sheet from '#lib/components/ui/sheet/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	let {
		baseline,
		onLoadOptions,
		onSave,
		onClose,
		onLatest
	}: {
		baseline: CurriculumQuickActionBaseline;
		onLoadOptions: () => Promise<CurriculumManagementOptions>;
		onSave: (requirements: CurriculumStructureRequirementInput[]) => Promise<void>;
		onClose: () => void;
		onLatest: () => Promise<void>;
	} = $props();
	const labels = {
		'add-course': 'เพิ่มรายวิชา',
		'add-activity': 'เพิ่มกิจกรรม',
		edit: 'แก้ไขเงื่อนไข',
		remove: 'นำรายการออก'
	};
	let adding = $derived(
		baseline.action.kind === 'add-course' || baseline.action.kind === 'add-activity'
	);
	let title = $derived(labels[baseline.action.kind]);
	let destination = $derived(
		`${baseline.programName} · ${baseline.gradeName} · ${baseline.termName}`
	);
	function initialKind() {
		return baseline.target?.requirementKind ?? 'required';
	}
	function initialSection() {
		return baseline.action.kind === 'add-course' &&
			baseline.action.context.section !== 'student_development'
			? (baseline.action.context.section ?? 'all')
			: 'all';
	}
	let requirementKind =
		$state<CurriculumStructureRequirementInput['requirementKind']>(initialKind());
	let section = $state(initialSection());
	let search = $state('');
	let selectedIds = $state<string[]>([]);
	let options = $state.raw<CurriculumManagementOptions | null>(null);
	let loading = $state(false);
	let loadError = $state('');
	let saving = $state(false);
	let saveError = $state('');
	let conflict = $state(false);
	let discardOpen = $state(false);
	let closeIntent = $state<'close' | 'latest'>('close');
	let current = true;
	let loadRevision = 0;
	let dirty = $derived(
		selectedIds.length > 0 || requirementKind !== (baseline.target?.requirementKind ?? 'required')
	);
	let choices = $derived.by(() => {
		const query = search.trim().toLocaleLowerCase('th');
		const kind = baseline.action.kind === 'add-course' ? 'course' : 'activity';
		return (options?.catalogVersions ?? [])
			.filter(
				(option) =>
					option.resourceKind === kind &&
					selectableCatalogOption(option, baseline.action.context.gradeLevelId) &&
					(section === 'all' || option.section === section) &&
					(!query || `${option.code} ${option.name}`.toLocaleLowerCase('th').includes(query))
			)
			.map((option) => ({
				option,
				existing: hasExistingCatalogRequirement(
					option,
					baseline.action.context,
					stageCurriculumRequirements(baseline.requirements),
					options?.catalogVersions ?? [],
					baseline.requirements
				)
			}));
	});
	async function loadOptions() {
		if (loading || saving) return;
		const revision = ++loadRevision;
		loading = true;
		loadError = '';
		try {
			const result = await onLoadOptions();
			if (current && revision === loadRevision) options = result;
		} catch (error) {
			if (current && revision === loadRevision && !isAbortError(error))
				loadError = error instanceof Error ? error.message : 'โหลดทะเบียนไม่สำเร็จ';
		} finally {
			if (current && revision === loadRevision) loading = false;
		}
	}
	onMount(() => {
		if (adding) void loadOptions();
		return () => {
			current = false;
			loadRevision++;
		};
	});
	function close(intent: 'close' | 'latest' = 'close') {
		if (saving) return;
		closeIntent = intent;
		if (dirty || conflict || intent === 'latest') discardOpen = true;
		else onClose();
	}
	async function discard() {
		discardOpen = false;
		if (closeIntent === 'close') {
			onClose();
			return;
		}
		saving = true;
		saveError = '';
		try {
			await onLatest();
		} catch (error) {
			if (current && !isAbortError(error))
				saveError = error instanceof Error ? error.message : 'โหลดข้อมูลล่าสุดไม่สำเร็จ';
		} finally {
			if (current) saving = false;
		}
	}
	async function save() {
		if (saving || conflict || (adding && (!options || !selectedIds.length))) return;
		saving = true;
		saveError = '';
		try {
			await onSave(
				quickActionRequirements(baseline, {
					requirementKind,
					selectedIds,
					options: options?.catalogVersions
				})
			);
			if (current) onClose();
		} catch (error) {
			if (!current || isAbortError(error)) return;
			conflict = error instanceof ApiClientError && error.status === 409;
			saveError = error instanceof Error ? error.message : 'บันทึกไม่สำเร็จ';
		} finally {
			if (current) saving = false;
		}
	}
</script>

{#snippet kindPicker()}
	<div class="space-y-2">
		<Label for="quick-requirement-kind">เงื่อนไข</Label>
		<Select.Root
			type="single"
			value={requirementKind}
			onValueChange={(value) => {
				if (value === 'required' || value === 'elective' || value === 'optional')
					requirementKind = value;
			}}
			disabled={saving || conflict}
		>
			<Select.Trigger id="quick-requirement-kind" aria-label="เงื่อนไข"
				>{requirementKind === 'required'
					? 'บังคับ'
					: requirementKind === 'elective'
						? 'เลือก'
						: 'เพิ่มเติม'}</Select.Trigger
			>
			<Select.Content
				><Select.Item value="required">บังคับ</Select.Item><Select.Item value="elective"
					>เลือก</Select.Item
				><Select.Item value="optional">เพิ่มเติม</Select.Item></Select.Content
			>
		</Select.Root>
	</div>
{/snippet}
{#snippet feedback()}
	{#if saveError}<PageState variant="error" title="บันทึกไม่สำเร็จ" description={saveError} />{/if}
	{#if conflict}<p class="text-sm text-muted-foreground">
			ร่างนี้เปลี่ยนไปแล้ว กรุณากลับไปตรวจข้อมูลล่าสุดก่อนทำรายการใหม่ การเลือกของคุณยังอยู่
		</p>
		<Button variant="outline" onclick={() => close('latest')} disabled={saving}
			>กลับไปดูข้อมูลล่าสุด</Button
		>{/if}
{/snippet}
{#snippet footer()}
	<Button variant="outline" onclick={() => close()} disabled={saving}>ยกเลิก</Button>
	<LoadingButton
		loading={saving}
		variant={baseline.action.kind === 'remove' ? 'destructive' : 'default'}
		onclick={save}
		disabled={conflict || (adding && (!options || !selectedIds.length))}
		>{baseline.action.kind === 'remove'
			? 'นำออกจากร่างนี้'
			: adding
				? `เพิ่ม ${selectedIds.length} รายการ`
				: 'บันทึก'}</LoadingButton
	>
{/snippet}
{#if adding}
	<Sheet.Root
		open
		onOpenChange={(open) => {
			if (!open) close();
		}}
	>
		<Sheet.Content
			class="w-full max-w-full gap-0 p-0 sm:w-[600px] sm:max-w-[min(600px,100vw)]"
			showCloseButton={false}
			onEscapeKeydown={(event) => {
				event.preventDefault();
				close();
			}}
			onInteractOutside={(event) => {
				event.preventDefault();
				close();
			}}
		>
			<Sheet.Header class="shrink-0 border-b p-5"
				><Sheet.Title>{title}</Sheet.Title><Sheet.Description>{destination}</Sheet.Description>
				<div class="space-y-3 pt-2">
					<Label for="quick-catalog-search">ค้นหารหัสหรือชื่อ</Label><Input
						id="quick-catalog-search"
						bind:value={search}
						placeholder="ค้นหารหัสหรือชื่อ"
						disabled={saving}
					/>
					{#if baseline.action.kind === 'add-course'}<Select.Root
							type="single"
							bind:value={section}
							disabled={saving}
							><Select.Trigger aria-label="หมวดทะเบียน"
								>{section === 'basic_course'
									? 'รายวิชาพื้นฐาน'
									: section === 'additional_course'
										? 'รายวิชาเพิ่มเติม'
										: 'ทุกหมวดรายวิชา'}</Select.Trigger
							><Select.Content
								><Select.Item value="all">ทุกหมวดรายวิชา</Select.Item><Select.Item
									value="basic_course">รายวิชาพื้นฐาน</Select.Item
								><Select.Item value="additional_course">รายวิชาเพิ่มเติม</Select.Item
								></Select.Content
							></Select.Root
						>{/if}
				</div>
			</Sheet.Header>
			<div class="min-h-0 flex-1 overflow-y-auto p-5" aria-label="เลือกจากทะเบียน">
				{#if loading && !options}<PageSkeleton
						variant="form"
						rows={5}
					/>{:else if loadError}<PageState
						variant="error"
						title="โหลดทะเบียนไม่สำเร็จ"
						description={loadError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadOptions}
					/>{:else if options}
					{#if choices.length}<div class="space-y-1">
							{#each choices as { option, existing } (option.id)}
								<label class="flex items-start gap-3 rounded-md p-2 hover:bg-muted/60">
									<Checkbox
										checked={selectedIds.includes(option.id)}
										disabled={existing || saving || conflict}
										aria-label={`เลือก ${option.resourceKind === 'course' ? option.code + ' ' : ''}${option.name} · ${formatEffectiveFrom(option.effectiveFrom)}`}
										onCheckedChange={(checked) => {
											selectedIds =
												checked === true
													? [...selectedIds, option.id]
													: selectedIds.filter((id) => id !== option.id);
										}}
									/>
									<span class="min-w-0"
										><span class="block text-xs font-medium text-primary"
											>{option.resourceKind === 'course' ? option.code : ''}</span
										><span class="block text-sm">{option.name}</span><span
											class="block text-xs text-muted-foreground"
											>{formatEffectiveFrom(option.effectiveFrom)}</span
										>{#if existing}<span class="block text-xs text-muted-foreground"
												>มีรายการนี้แล้ว</span
											>{/if}</span
									>
								</label>
							{/each}
						</div>{:else}<PageState
							variant="empty"
							title={search || section !== 'all'
								? 'ไม่พบรายการที่ตรงกับตัวกรอง'
								: 'ไม่มีรายการที่เลือกเพิ่มได้'}
							description="ทะเบียนปัจจุบันต้องครอบคลุมชั้นปลายทาง"
						/>{/if}{/if}
			</div>
			<Sheet.Footer class="shrink-0 border-t p-5"
				><div class="w-full space-y-3">
					<p class="text-sm" aria-live="polite">เลือก {selectedIds.length} รายการ</p>
					{@render kindPicker()}{@render feedback()}
					<div class="flex justify-end gap-2">{@render footer()}</div>
				</div></Sheet.Footer
			>
		</Sheet.Content>
	</Sheet.Root>
{:else if baseline.action.kind === 'remove'}
	<AlertDialog.Root
		open
		onOpenChange={(open) => {
			if (!open) close();
		}}
		><AlertDialog.Content
			onEscapeKeydown={(event) => {
				event.preventDefault();
				close();
			}}
			><AlertDialog.Header
				><AlertDialog.Title>{title}</AlertDialog.Title><AlertDialog.Description
					>{destination}</AlertDialog.Description
				></AlertDialog.Header
			>
			<p class="font-medium">
				{baseline.target?.resourceKind === 'course' ? baseline.target.code + ' ' : ''}{baseline
					.target?.name}
			</p>
			<p class="text-sm text-muted-foreground">นำรายการนี้ออกจากร่างตามปลายทางข้างต้น</p>
			{@render feedback()}<AlertDialog.Footer>{@render footer()}</AlertDialog.Footer
			></AlertDialog.Content
		></AlertDialog.Root
	>
{:else}
	<Dialog.Root
		open
		onOpenChange={(open) => {
			if (!open) close();
		}}
	>
		<Dialog.Content
			showCloseButton={false}
			onEscapeKeydown={(event) => {
				event.preventDefault();
				close();
			}}
			onInteractOutside={(event) => {
				event.preventDefault();
				close();
			}}
		>
			<Dialog.Header
				><Dialog.Title>{title}</Dialog.Title><Dialog.Description>{destination}</Dialog.Description
				></Dialog.Header
			>
			<div class="space-y-3">
				<p class="font-medium">
					{baseline.target?.resourceKind === 'course' ? baseline.target.code + ' ' : ''}{baseline
						.target?.name}
				</p>
				{#if baseline.action.kind === 'edit'}{@render kindPicker()}
					<p class="text-sm text-muted-foreground">
						ค่าทางการจากทะเบียน · {curriculumMetric(baseline.target?.metrics.credit)} หน่วยกิต · {curriculumMetric(
							baseline.target?.metrics.weeklyValue
						)} ชั่วโมง/สัปดาห์ · {curriculumMetric(baseline.target?.metrics.totalHours)} ชั่วโมง/ภาคเรียน
					</p>{:else}<p class="text-sm text-muted-foreground">
						นำรายการนี้ออกจากร่างตามปลายทางข้างต้น
					</p>{/if}{@render feedback()}
			</div>
			<Dialog.Footer>{@render footer()}</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
{/if}
<AlertDialog.Root bind:open={discardOpen}
	><AlertDialog.Content
		><AlertDialog.Header
			><AlertDialog.Title>ละทิ้งการเปลี่ยนแปลง?</AlertDialog.Title><AlertDialog.Description
				>{closeIntent === 'latest'
					? 'การเลือกนี้จะถูกละทิ้ง แล้วกลับไปดูข้อมูลล่าสุด'
					: 'การเปลี่ยนแปลงที่ยังไม่บันทึกจะถูกละทิ้ง'}</AlertDialog.Description
			></AlertDialog.Header
		><AlertDialog.Footer
			><AlertDialog.Cancel>แก้ไขต่อ</AlertDialog.Cancel><AlertDialog.Action onclick={discard}
				>ละทิ้งการเปลี่ยนแปลง</AlertDialog.Action
			></AlertDialog.Footer
		></AlertDialog.Content
	></AlertDialog.Root
>
