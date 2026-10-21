<script lang="ts">
	import * as Card from '#lib/components/ui/card/index.js';
	import type { BudgetCatalogs } from '#lib/api/budget.js';
	import {
		type ActivityDraft,
		newLine,
		catalogLine,
		applyDefaults,
		previewTotal
	} from '#lib/budget/drafts.js';
	import { sumSatang, money } from '#lib/budget/money.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import Choice from './Choice.svelte';
	import LineEditor from './LineEditor.svelte';
	import CatalogPicker from './CatalogPicker.svelte';
	let {
		activity = $bindable(),
		catalogs,
		submitted = false,
		disabled = false,
		onremove
	}: {
		activity: ActivityDraft;
		catalogs: BudgetCatalogs;
		submitted?: boolean;
		disabled?: boolean;
		onremove: () => void;
	} = $props();
	let selected = $state<string[]>([]),
		catalogOpen = $state(false);
	const id = $derived(`budget-activity-${activity.key}`);
</script>

<Card.Root class="gap-0" aria-label={activity.title || 'กิจกรรมใหม่'}
	><Card.Content class="space-y-5">
		<div class="flex flex-wrap items-end justify-between gap-3">
			<div class="min-w-0 flex-1 space-y-2">
				<Label for={`${id}-title`}>ชื่อกิจกรรม *</Label><Input
					id={`${id}-title`}
					bind:value={activity.title}
					{disabled}
					aria-invalid={submitted && !activity.title.trim()}
				/>{#if submitted && !activity.title.trim()}<p class="text-sm text-destructive">
						กรอกชื่อกิจกรรม
					</p>{/if}
			</div>
			<Button type="button" variant="ghost" {disabled} onclick={onremove}>ลบกิจกรรม</Button>
		</div>
		<div class="space-y-4 rounded-lg bg-muted/40 p-4">
			<p class="text-sm font-medium">ค่าเริ่มต้นสำหรับรายการใหม่</p>
			<div class="grid gap-3 sm:grid-cols-2">
				<Choice
					id={`${id}-fund`}
					label="แหล่งเงินเริ่มต้น"
					value={activity.defaultFundId ?? ''}
					onchange={(value) => (activity.defaultFundId = value)}
					options={[
						{ value: '', label: 'ไม่กำหนด' },
						...catalogs.funds
							.filter((item) => item.active || item.id === activity.defaultFundId)
							.map((item) => ({ value: item.id, label: item.name }))
					]}
					{disabled}
				/><Choice
					id={`${id}-category`}
					label="หมวดรายจ่ายเริ่มต้น"
					value={activity.defaultExpenseCategoryId ?? ''}
					onchange={(value) => (activity.defaultExpenseCategoryId = value)}
					options={[
						{ value: '', label: 'ไม่กำหนด' },
						...catalogs.expenseCategories
							.filter((item) => item.active || item.id === activity.defaultExpenseCategoryId)
							.map((item) => ({ value: item.id, label: item.name }))
					]}
					{disabled}
				/>
			</div>
			{#if activity.lines.length}<div class="flex flex-wrap items-center justify-between gap-2">
					<p class="text-sm text-muted-foreground">
						เลือกแถวที่ต้องการเปลี่ยนแหล่งเงินและหมวดรายจ่าย
					</p>
					<Button
						type="button"
						size="sm"
						variant="outline"
						disabled={disabled ||
							!selected.length ||
							(!activity.defaultFundId && !activity.defaultExpenseCategoryId)}
						onclick={() => {
							activity = applyDefaults(activity, selected);
							selected = [];
						}}>ใช้ค่าเริ่มต้นกับ {selected.length} รายการที่เลือก</Button
					>
				</div>{/if}
		</div>
		{#each activity.lines as line, index (line.key)}<div class="space-y-3 border-t pt-5">
				<div class="flex items-center gap-2">
					<Checkbox
						id={`${id}-select-${line.key}`}
						checked={selected.includes(line.key)}
						{disabled}
						onCheckedChange={(checked) =>
							(selected = checked
								? [...selected, line.key]
								: selected.filter((key) => key !== line.key))}
					/><Label for={`${id}-select-${line.key}`}
						>เลือกรายการที่ {index + 1} เพื่อเปลี่ยนค่า</Label
					>
				</div>
				<LineEditor
					bind:line={activity.lines[index]}
					{catalogs}
					{submitted}
					{disabled}
					onremove={() => {
						activity.lines = activity.lines.filter((value) => value.key !== line.key);
						selected = selected.filter((key) => key !== line.key);
					}}
				/>
			</div>{/each}
		<div class="flex flex-wrap items-center gap-2">
			<Button
				type="button"
				variant="outline"
				{disabled}
				onclick={() => (activity.lines = [...activity.lines, newLine(activity)])}
				>เพิ่มรายการกำหนดเอง</Button
			>{#if $can.has(PERMISSIONS.SUPPLY_CATALOG_READ_SCHOOL)}<Button
					type="button"
					variant="outline"
					{disabled}
					onclick={() => (catalogOpen = true)}>เลือกพัสดุจากคลัง</Button
				>{/if}
			<p class="ml-auto text-sm tabular-nums">
				รวมกิจกรรม <strong>{money(sumSatang(activity.lines.map(previewTotal)))} บาท</strong>
			</p>
		</div>
	</Card.Content></Card.Root
>
<Dialog.Root bind:open={catalogOpen}
	><Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-2xl"
		><Dialog.Header
			><Dialog.Title>เลือกพัสดุจากคลัง</Dialog.Title><Dialog.Description
				>เลือกรุ่นราคาสำหรับรายการค่าใช้จ่ายนี้</Dialog.Description
			></Dialog.Header
		>{#if catalogOpen}<CatalogPicker
				onselect={(item) => {
					activity.lines = [...activity.lines, catalogLine(activity, item)];
					catalogOpen = false;
				}}
			/>{/if}</Dialog.Content
	></Dialog.Root
>
