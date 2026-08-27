<script module lang="ts">
	export type CatalogVersionItem = {
		id: string;
		versionNo: number;
		name: string;
		secondaryName?: string | null;
		exactValue: string;
		effectiveFrom: string;
		effectiveUntil?: string | null;
		classification?: string | null;
		gradeLevels?: import('$lib/api/academic-core').GradeLevelOption[];
		status: 'draft' | 'published' | 'archived';
		rowVersion: number;
	};

	export type CatalogVersionDraft = {
		name: string;
		secondaryName: string;
		exactValue: string;
		effectiveFrom: string;
		effectiveUntil: string;
		gradeLevelIds: string[];
		classification: string;
	};
</script>

<script lang="ts">
	import type { GradeLevelOption } from '$lib/api/academic-core';
	import {
		SCHEDULING_MODE_OPTIONS,
		SUBJECT_TYPE_OPTIONS,
		formatEffectiveRange,
		gradeLevelSummary,
		optionLabel,
		versionStatusLabel
	} from '$lib/academic-core/catalog-presentation';
	import GradeLevelMultiSelect from '$lib/components/academic-core/GradeLevelMultiSelect.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { CheckCircle2, GitBranchPlus, History } from 'lucide-svelte';

	let {
		kind,
		code,
		items,
		gradeLevelOptions = [],
		canManage = false,
		onCreate,
		onPublish
	}: {
		kind: 'subject' | 'activity';
		code: string;
		items: CatalogVersionItem[];
		gradeLevelOptions?: GradeLevelOption[];
		canManage?: boolean;
		onCreate: (draft: CatalogVersionDraft) => Promise<void>;
		onPublish: (id: string, rowVersion: number) => Promise<void>;
	} = $props();

	let draft = $state<CatalogVersionDraft>({
		name: '',
		secondaryName: '',
		exactValue: '1.00',
		effectiveFrom: '',
		effectiveUntil: '',
		gradeLevelIds: [],
		classification: ''
	});
	let busy = $state(false);
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
		if (!draft.effectiveFrom) {
			errorMessage = 'กรุณาเลือกวันที่เริ่มใช้';
			return;
		}
		busy = true;
		errorMessage = '';
		try {
			await onCreate({ ...draft, classification: selectedClassification });
			draft = { ...draft, name: '', secondaryName: '', effectiveUntil: '' };
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'สร้างรุ่นใหม่ไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}

	async function publish(item: CatalogVersionItem) {
		busy = true;
		errorMessage = '';
		try {
			await onPublish(item.id, item.rowVersion);
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'เผยแพร่รุ่นไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}
</script>

<div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
	<section class="rounded-xl border bg-card">
		<header class="flex items-center gap-2 border-b px-5 py-4">
			<History class="size-5 text-primary" />
			<div>
				<h2 class="font-semibold">ประวัติรุ่น · {code}</h2>
				<p class="text-xs text-muted-foreground">รุ่นที่เผยแพร่แล้วจะไม่ถูกแก้ทับ</p>
			</div>
		</header>
		<div class="divide-y">
			{#each items as item (item.id)}
				<article class="grid gap-3 px-5 py-4 sm:grid-cols-[52px_1fr_auto] sm:items-center">
					<div class="text-center">
						<p class="text-lg font-semibold tabular-nums">v{item.versionNo}</p>
						<p class="text-[10px] uppercase text-muted-foreground">version</p>
					</div>
					<div>
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
							<span>{kind === 'subject' ? 'หน่วยกิต' : 'ชม./สัปดาห์'} {item.exactValue}</span>
							{#if item.classification}
								<span>{optionLabel(classificationOptions, item.classification)}</span>
							{/if}
							<span>{formatEffectiveRange(item.effectiveFrom, item.effectiveUntil)}</span>
							{#if item.gradeLevels}
								<span>{gradeLevelSummary(item.gradeLevels)}</span>
							{/if}
						</div>
					</div>
					{#if canManage && item.status === 'draft'}<Button
							size="sm"
							variant="outline"
							disabled={busy}
							onclick={() => publish(item)}><CheckCircle2 class="size-4" /> เผยแพร่</Button
						>{/if}
				</article>
			{:else}
				<p class="p-8 text-center text-sm text-muted-foreground">ยังไม่มีรุ่นข้อมูล</p>
			{/each}
		</div>
	</section>

	{#if canManage}
		<form class="space-y-3 rounded-xl border bg-card p-5" onsubmit={submit}>
			<div class="flex items-center gap-2">
				<GitBranchPlus class="size-5 text-primary" />
				<h2 class="font-semibold">สร้างรุ่นใหม่</h2>
			</div>
			<div class="space-y-1.5">
				<Label for={`${code}-version-name`}>ชื่อภาษาไทย</Label><Input
					id={`${code}-version-name`}
					bind:value={draft.name}
					required
				/>
			</div>
			<div class="space-y-1.5">
				<Label for={`${code}-version-en`}>ชื่อภาษาอังกฤษ</Label><Input
					id={`${code}-version-en`}
					bind:value={draft.secondaryName}
				/>
			</div>
			<div class="space-y-1.5">
				<Label for={`${code}-version-exact`}
					>{kind === 'subject' ? 'หน่วยกิต' : 'ชั่วโมงต่อสัปดาห์'}</Label
				><Input
					id={`${code}-version-exact`}
					inputmode="decimal"
					bind:value={draft.exactValue}
					required
				/>
			</div>
			<div class="space-y-1.5">
				<Label for={`${code}-version-class`}
					>{kind === 'subject' ? 'ประเภทรายวิชา' : 'รูปแบบจัดกิจกรรม'}</Label
				>
				<Select.Root
					type="single"
					value={selectedClassification}
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
					ariaLabel={`เลือกระดับชั้นสำหรับ ${code}`}
				/>
				<p class="text-xs text-muted-foreground">ไม่เลือก หมายถึงใช้ได้กับทุกระดับชั้น</p>
			</div>
			<div class="grid grid-cols-2 gap-3">
				<div class="space-y-1.5">
					<Label for={`${code}-version-from`}>เริ่มใช้</Label>
					<DatePicker
						id={`${code}-version-from`}
						bind:value={draft.effectiveFrom}
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
						placeholder="ไม่กำหนดวันสิ้นสุด"
						ariaLabel="เลือกวันที่สิ้นสุด"
						clearable
					/>
				</div>
			</div>
			<Button class="w-full" type="submit" disabled={busy}
				><GitBranchPlus class="size-4" /> บันทึกร่างรุ่นใหม่</Button
			>
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		</form>
	{/if}
</div>
