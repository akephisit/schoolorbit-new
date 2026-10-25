<script module lang="ts">
	export type CatalogVersionItem = {
		id: string;
		versionNo: number;
		name: string;
		secondaryName?: string | null;
		exactValue: string;
		totalValue?: string | null;
		standardPeriodsPerWeek?: number | null;
		effectiveFrom: string;
		effectiveUntil?: string | null;
		classification?: string | null;
		gradeLevels?: import('#lib/api/academic-core.js').GradeLevelOption[];
		status: 'draft' | 'published' | 'archived';
		rowVersion: number;
	};

	export type CatalogVersionDraft =
		import('#lib/academic-core/catalog-version-draft.js').CatalogVersionDraft;
</script>

<script lang="ts">
	import { untrack } from 'svelte';
	import { createCatalogVersionDraft } from '#lib/academic-core/catalog-version-draft.js';
	import type { SubjectVersion } from '#lib/api/academic-core.js';
	import type { GradeLevelOption } from '#lib/api/academic-core.js';
	import {
		SCHEDULING_MODE_OPTIONS,
		SUBJECT_TYPE_OPTIONS,
		formatEffectiveRange,
		gradeLevelSummary,
		optionLabel,
		versionStatusLabel
	} from '#lib/academic-core/catalog-presentation.js';
	import GradeLevelMultiSelect from '#lib/components/academic-core/GradeLevelMultiSelect.svelte';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { DatePicker } from '#lib/components/ui/date-picker/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { CheckCircle2, GitBranchPlus, History } from '@lucide/svelte';

	let {
		kind,
		code,
		items,
		gradeLevelOptions = [],
		sourceVersion = null,
		canManage = false,
		onCreate,
		onPublish
	}: {
		kind: 'subject' | 'activity';
		code: string;
		items: CatalogVersionItem[];
		gradeLevelOptions?: GradeLevelOption[];
		sourceVersion?: SubjectVersion | null;
		canManage?: boolean;
		onCreate: (draft: CatalogVersionDraft) => Promise<void>;
		onPublish: (id: string, rowVersion: number) => Promise<void>;
	} = $props();

	// Snapshot only when this keyed history mounts; refreshed props must not overwrite edits.
	let draft = $state<CatalogVersionDraft>(
		untrack(() => createCatalogVersionDraft(kind === 'subject' ? sourceVersion : null))
	);
	let sourceRevision = $state(
		untrack(() => (kind === 'subject' ? sourceVersion?.versionNo : null))
	);
	// Keep canonical decimals as strings, including intermediate typing such as `1.`.
	const positiveDecimalPattern = '(?=.*[1-9])(0|[1-9][0-9]*)(\\.[0-9]{1,2})?';
	const nonnegativeDecimalPattern = '(0|[1-9][0-9]*)(\\.[0-9]{1,2})?';
	const positiveIntegerPattern = '(?=.*[1-9])[0-9]+(\\.0+)?';

	let pendingAction = $state<string | null>(null);
	let busy = $derived(pendingAction !== null);
	let errorAction = $state<'create' | 'publish'>('create');
	let errorMessage = $state('');
	let classificationOptions = $derived(
		kind === 'subject' ? SUBJECT_TYPE_OPTIONS : SCHEDULING_MODE_OPTIONS
	);
	let selectedClassification = $derived(
		classificationOptions.some((option) => option.value === draft.classification)
			? draft.classification
			: classificationOptions[0].value
	);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (busy || !canManage || !(event.currentTarget as HTMLFormElement).reportValidity()) return;
		errorAction = 'create';
		if (!draft.effectiveFrom) {
			errorMessage = 'กรุณาเลือกวันที่เริ่มใช้';
			return;
		}
		pendingAction = 'create';
		errorMessage = '';
		try {
			await onCreate({ ...draft, classification: selectedClassification });
			if (kind === 'subject') {
				draft = createCatalogVersionDraft();
				sourceRevision = null;
			} else draft = { ...draft, name: '', secondaryName: '', effectiveUntil: '' };
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างรุ่นใหม่ไม่สำเร็จ';
		} finally {
			pendingAction = null;
		}
	}

	async function publish(item: CatalogVersionItem) {
		if (busy || !canManage || item.status !== 'draft') return;
		errorAction = 'publish';
		pendingAction = item.id;
		errorMessage = '';
		try {
			await onPublish(item.id, item.rowVersion);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'เผยแพร่รุ่นไม่สำเร็จ';
		} finally {
			pendingAction = null;
		}
	}
</script>

<div class="grid min-w-0 items-start gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(0,24rem)]">
	<section class="min-w-0 rounded-xl border bg-card" aria-label="ประวัติรุ่น">
		<header class="flex items-center gap-2 border-b p-3 sm:p-4">
			<History class="size-5 text-primary" />
			<div>
				<h2 class="font-semibold">ประวัติรุ่น{kind === 'subject' ? ` · ${code}` : 'กิจกรรม'}</h2>
				<p class="text-xs text-muted-foreground">รุ่นที่เผยแพร่แล้วจะไม่ถูกแก้ทับ</p>
			</div>
		</header>
		{#if errorMessage && errorAction === 'publish'}<p
				role="alert"
				class="border-b p-3 text-sm text-destructive"
			>
				{errorMessage}
			</p>{/if}
		<div class="divide-y">
			{#each items as item (item.id)}
				<article
					class="grid min-w-0 gap-3 p-3 sm:grid-cols-[auto_minmax(0,1fr)_auto] sm:items-start sm:p-4"
				>
					<div>
						<p class="text-lg font-semibold tabular-nums">v{item.versionNo}</p>
					</div>
					<div class="min-w-0">
						<div class="flex flex-wrap items-center gap-2">
							<h3 class="font-medium">{item.name}</h3>
							<Badge variant={item.status === 'published' ? 'default' : 'secondary'}
								>{versionStatusLabel(item.status)}</Badge
							>
						</div>
						{#if item.secondaryName}<p class="text-xs text-muted-foreground">
								{item.secondaryName}
							</p>{/if}
						<div class="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
							{#if kind === 'subject'}
								<span>หน่วยกิต {item.exactValue}</span>
								<span>มาตรฐาน {item.standardPeriodsPerWeek ?? '—'} คาบ/สัปดาห์</span>
								<span>รวม {item.totalValue ?? '—'} ชม./ภาคเรียน</span>
							{:else}
								<span>ชม./สัปดาห์ {item.exactValue}</span>
								<span>รวม {item.totalValue ?? '—'} ชม./ภาคเรียน</span>
							{/if}
							{#if item.classification}
								<span>{optionLabel(classificationOptions, item.classification)}</span>
							{/if}
							<span>{formatEffectiveRange(item.effectiveFrom, item.effectiveUntil)}</span>
							{#if item.gradeLevels}
								<span>{gradeLevelSummary(item.gradeLevels)}</span>
							{/if}
						</div>
					</div>
					{#if canManage && item.status === 'draft'}<LoadingButton
							size="sm"
							variant="outline"
							disabled={busy}
							loading={pendingAction === item.id}
							loadingLabel="กำลังเผยแพร่..."
							onclick={() => publish(item)}><CheckCircle2 class="size-4" /> เผยแพร่</LoadingButton
						>{/if}
				</article>
			{:else}
				<p class="p-8 text-center text-sm text-muted-foreground">ยังไม่มีรุ่นข้อมูล</p>
			{/each}
		</div>
	</section>

	{#if canManage}
		<form
			class="min-w-0 space-y-4 rounded-xl border bg-card p-3 sm:p-4"
			onsubmit={submit}
			aria-label="สร้างรุ่นใหม่"
		>
			<div class="flex items-center gap-2">
				<GitBranchPlus class="size-5 text-primary" />
				<h2 class="font-semibold">สร้างรุ่นใหม่</h2>
			</div>
			{#if sourceRevision}<p class="text-xs text-muted-foreground">
					เริ่มจากรุ่นที่เผยแพร่ v{sourceRevision} · ปรับข้อมูลแล้วบันทึกเป็นรุ่นใหม่
				</p>{/if}
			<fieldset disabled={busy} class="min-w-0 space-y-3">
				<div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-1">
					<div class="space-y-1.5">
						<Label for={`${code}-version-name`}>ชื่อภาษาไทย</Label><Input
							id={`${code}-version-name`}
							bind:value={draft.name}
							required
						/>
					</div>
					<div class="space-y-1.5">
						<Label for={`${code}-version-en`}
							>{kind === 'subject' ? 'ชื่อภาษาอังกฤษ' : 'คำอธิบาย'}</Label
						><Input id={`${code}-version-en`} bind:value={draft.secondaryName} />
					</div>
				</div>
				<div class="space-y-3 rounded-xl border border-primary/15 bg-primary/5 p-3">
					<div>
						<p class="text-sm font-medium">
							{kind === 'subject' ? 'ภาระการเรียนตามหลักสูตร' : 'ชั่วโมงกิจกรรมทางการ'}
						</p>
						<p class="text-xs text-muted-foreground">
							ค่าชุดนี้จะแสดงในหลักสูตร และใช้เป็นค่าเริ่มต้นเมื่อเปิดสอน
						</p>
					</div>
					<div class="grid gap-3 sm:grid-cols-3 xl:grid-cols-2">
						<div class="space-y-1.5">
							<Label for={`${code}-version-exact`}>
								{kind === 'subject' ? 'หน่วยกิต' : 'ชั่วโมงต่อสัปดาห์'}
							</Label>
							<Input
								id={`${code}-version-exact`}
								type="text"
								pattern={kind === 'subject' ? positiveDecimalPattern : nonnegativeDecimalPattern}
								inputmode="decimal"
								bind:value={draft.exactValue}
								required
							/>
						</div>
						{#if kind === 'subject'}
							<div class="space-y-1.5">
								<Label for={`${code}-version-periods`}>คาบมาตรฐานต่อสัปดาห์</Label>
								<Input
									id={`${code}-version-periods`}
									type="text"
									pattern={positiveIntegerPattern}
									inputmode="numeric"
									bind:value={draft.standardPeriodsPerWeek}
									required
								/>
							</div>
						{/if}
						<div class="space-y-1.5">
							<Label for={`${code}-version-total`}>ชั่วโมงรวมต่อภาคเรียน</Label>
							<Input
								id={`${code}-version-total`}
								type="text"
								pattern={kind === 'subject' ? positiveIntegerPattern : nonnegativeDecimalPattern}
								inputmode={kind === 'subject' ? 'numeric' : 'decimal'}
								bind:value={draft.totalValue}
								required
							/>
						</div>
					</div>
				</div>
				<div class="space-y-1.5">
					<Label for={`${code}-version-class`}
						>{kind === 'subject' ? 'ประเภทรายวิชา' : 'รูปแบบจัดกิจกรรม'}</Label
					>
					<Select.Root
						type="single"
						value={selectedClassification}
						disabled={busy}
						onValueChange={(value) => (draft.classification = value)}
					>
						<Select.Trigger id={`${code}-version-class`} class="w-full">
							{optionLabel(classificationOptions, selectedClassification, 'เลือกประเภท')}
						</Select.Trigger>
						<Select.Content>
							{#each classificationOptions as option (option.value)}
								<Select.Item value={option.value}>{option.label}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<div class="space-y-1.5">
					<Label>ระดับชั้นที่ใช้</Label>
					<GradeLevelMultiSelect
						bind:value={draft.gradeLevelIds}
						options={gradeLevelOptions}
						disabled={busy}
						ariaLabel={`เลือกระดับชั้นสำหรับ ${code}`}
					/>
					<p class="text-xs text-muted-foreground">ไม่เลือก หมายถึงใช้ได้กับทุกระดับชั้น</p>
				</div>
				<div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-1">
					<div class="space-y-1.5">
						<Label for={`${code}-version-from`}>เริ่มใช้</Label>
						<DatePicker
							id={`${code}-version-from`}
							bind:value={draft.effectiveFrom}
							disabled={busy}
							placeholder="เลือกวันเริ่มใช้"
							ariaLabel="เลือกวันที่เริ่มใช้"
							required
						/>
					</div>
					<div class="space-y-1.5">
						<Label for={`${code}-version-until`}>สิ้นสุด</Label>
						<DatePicker
							id={`${code}-version-until`}
							bind:value={draft.effectiveUntil}
							disabled={busy}
							placeholder="ไม่กำหนดวันสิ้นสุด"
							ariaLabel="เลือกวันที่สิ้นสุด"
							clearable
						/>
					</div>
				</div>
			</fieldset>
			{#if errorMessage && errorAction === 'create'}<p
					role="alert"
					class="text-sm text-destructive"
				>
					{errorMessage}
				</p>{/if}
			<LoadingButton
				class="w-full"
				type="submit"
				loading={pendingAction === 'create'}
				disabled={busy}
				loadingLabel="กำลังบันทึกร่าง..."
				><GitBranchPlus class="size-4" aria-hidden="true" />บันทึกร่างรุ่นใหม่</LoadingButton
			>
		</form>
	{/if}
</div>
