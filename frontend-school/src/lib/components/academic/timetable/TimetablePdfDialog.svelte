<script lang="ts">
	import { untrack } from 'svelte';
	import {
		createTimetableBoardState,
		rowsForTimetableView,
		type TimetableBoardView,
		type TimetablePageView
	} from '#lib/academic/timetable/board-state.js';
	import type { TimetableBlockWorkspace } from '#lib/api/timetable.js';
	import type { GeneratePdfOptions } from '#lib/utils/pdf.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Download, LoaderCircle } from '@lucide/svelte';

	let {
		open = $bindable(false),
		workspace,
		view,
		ownerId,
		exporting,
		ready,
		onDownload
	}: {
		open?: boolean;
		workspace: TimetableBlockWorkspace;
		view: TimetablePageView;
		ownerId: string | null;
		exporting: boolean;
		ready: boolean;
		onDownload: (
			view: TimetableBoardView,
			ids: string[],
			layout: NonNullable<GeneratePdfOptions['layout']>
		) => Promise<void>;
	} = $props();

	let exportView = $state<TimetableBoardView>(
		untrack(() => (view === 'wholeSchool' ? 'homeroom' : view))
	);
	let selectedIds = $state<string[]>(
		untrack(() =>
			view === 'wholeSchool' ? workspace.homerooms.map((room) => room.id) : ownerId ? [ownerId] : []
		)
	);
	let layout = $state<NonNullable<GeneratePdfOptions['layout']>>('full');
	let search = $state('');
	const options = $derived(rowsForTimetableView(createTimetableBoardState(workspace), exportView));
	const selected = $derived(options.filter((option) => selectedIds.includes(option.id)));
	const filtered = $derived(
		options.filter((option) =>
			`${option.label} ${option.code}`
				.toLocaleLowerCase('th-TH')
				.includes(search.trim().toLocaleLowerCase('th-TH'))
		)
	);
	const types: { value: TimetableBoardView; label: string }[] = [
		{ value: 'homeroom', label: 'ห้อง' },
		{ value: 'learning_group', label: 'กลุ่มเรียน' },
		{ value: 'teacher', label: 'ครู' }
	];
</script>

<Dialog.Root
	{open}
	onOpenChange={(value) => {
		if (!exporting) open = value;
	}}
>
	<Dialog.Content
		class="flex max-h-[85dvh] flex-col overflow-hidden sm:max-w-xl"
		showCloseButton={!exporting}
		onEscapeKeydown={(event) => {
			if (exporting) event.preventDefault();
		}}
		onInteractOutside={(event) => {
			if (exporting) event.preventDefault();
		}}
	>
		<Dialog.Header>
			<Dialog.Title>ดาวน์โหลดตารางสอน PDF</Dialog.Title>
			<Dialog.Description>เลือกห้อง กลุ่มเรียน หรือครู แล้วเลือกรูปแบบจัดหน้า</Dialog.Description>
		</Dialog.Header>
		<div class="min-h-0 space-y-4 overflow-y-auto pr-1">
			<fieldset disabled={exporting} class="min-w-0 space-y-4">
				<div class="space-y-2">
					<p class="text-sm font-medium">ประเภทตาราง</p>
					<div class="flex gap-2" role="group" aria-label="ประเภทตาราง PDF">
						{#each types as type (type.value)}
							<Button
								class="flex-1"
								variant={exportView === type.value ? 'default' : 'outline'}
								aria-pressed={exportView === type.value}
								onclick={() => {
									exportView = type.value;
									selectedIds = [];
									search = '';
								}}>{type.label}</Button
							>
						{/each}
					</div>
				</div>
				<div class="space-y-2">
					<p class="text-sm font-medium">รูปแบบจัดหน้า</p>
					<div class="grid gap-2 sm:grid-cols-2" role="group" aria-label="รูปแบบจัดหน้า PDF">
						<Button
							variant={layout === 'full' ? 'default' : 'outline'}
							aria-pressed={layout === 'full'}
							onclick={() => (layout = 'full')}>1 ตารางต่อหน้า</Button
						>
						<Button
							variant={layout === 'portrait-2col' ? 'default' : 'outline'}
							aria-pressed={layout === 'portrait-2col'}
							onclick={() => (layout = 'portrait-2col')}>6 ตารางต่อหน้า</Button
						>
					</div>
					<p class="text-xs text-muted-foreground">
						{layout === 'full'
							? 'A4 แนวนอน แสดงรายละเอียดเต็ม'
							: 'A4 แนวตั้ง แบบย่อ 2 คอลัมน์ แสดงรหัสวิชา ครู และห้อง'}
					</p>
				</div>
				<div class="space-y-2">
					<div class="flex flex-wrap items-center justify-between gap-2">
						<p class="text-sm font-medium" role="status">
							เลือกแล้ว {selected.length} / {options.length} รายการ
						</p>
						<div class="flex gap-2">
							<Button
								size="sm"
								variant="outline"
								disabled={options.length === 0}
								onclick={() => (selectedIds = options.map((option) => option.id))}
								>เลือกทั้งหมด</Button
							>
							<Button
								size="sm"
								variant="ghost"
								disabled={selected.length === 0}
								onclick={() => (selectedIds = [])}>ล้างการเลือก</Button
							>
						</div>
					</div>
					<Label for="pdf-owner-search" class="sr-only">ค้นหารายการดาวน์โหลด</Label>
					<Input
						id="pdf-owner-search"
						bind:value={search}
						placeholder="ค้นหาห้อง กลุ่มเรียน หรือชื่อครู..."
					/>
					<div
						class="max-h-60 overflow-y-auto rounded-lg border"
						aria-label="รายการตารางที่ดาวน์โหลด"
					>
						{#each filtered as option (option.id)}
							<div class="flex items-center gap-3 border-b p-3 last:border-b-0">
								<Checkbox
									id={`pdf-owner-${option.id}`}
									checked={selectedIds.includes(option.id)}
									onCheckedChange={(checked) => {
										selectedIds = checked
											? [...selectedIds, option.id]
											: selectedIds.filter((id) => id !== option.id);
									}}
								/>
								<Label
									for={`pdf-owner-${option.id}`}
									class="min-w-0 flex-1 cursor-pointer leading-normal">{option.label}</Label
								>
							</div>
						{:else}<p class="p-4 text-sm text-muted-foreground">
								{options.length ? 'ไม่พบรายการที่ค้นหา' : 'ไม่มีรายการให้ดาวน์โหลด'}
							</p>{/each}
					</div>
				</div>
			</fieldset>
		</div>
		<Dialog.Footer class="shrink-0 border-t pt-4">
			<Button variant="outline" disabled={exporting} onclick={() => (open = false)}>ยกเลิก</Button>
			<Button
				disabled={!ready || selected.length === 0}
				onclick={() =>
					onDownload(
						exportView,
						selected.map((option) => option.id),
						layout
					)}
			>
				{#if exporting}<LoaderCircle class="size-4 animate-spin" />{:else}<Download
						class="size-4"
					/>{/if}
				{exporting ? 'กำลังดาวน์โหลด...' : `ดาวน์โหลด (${selected.length})`}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
