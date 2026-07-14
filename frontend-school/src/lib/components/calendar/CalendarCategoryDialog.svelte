<script lang="ts">
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Tabs from '$lib/components/ui/tabs';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import type {
		CalendarCategory,
		CalendarTag,
		UpsertCalendarCategoryRequest,
		UpsertCalendarTagRequest
	} from '$lib/api/calendar';
	import { cn } from '$lib/utils';
	import { Plus, Tag, Trash2 } from 'lucide-svelte';

	type DeleteCandidate =
		| { kind: 'category'; item: CalendarCategory }
		| { kind: 'tag'; item: CalendarTag };

	const colorOptions = ['#2563eb', '#16a34a', '#f59e0b', '#dc2626', '#7c3aed', '#0891b2'];

	let {
		open = $bindable(false),
		categories = [],
		tags = [],
		saving = false,
		onsavecategory,
		ondeletecategory,
		onsavetag,
		ondeletetag
	}: {
		open: boolean;
		categories?: CalendarCategory[];
		tags?: CalendarTag[];
		saving?: boolean;
		onsavecategory?: (
			id: string | null,
			payload: UpsertCalendarCategoryRequest
		) => Promise<boolean>;
		ondeletecategory?: (category: CalendarCategory) => Promise<boolean>;
		onsavetag?: (id: string | null, payload: UpsertCalendarTagRequest) => Promise<boolean>;
		ondeletetag?: (tag: CalendarTag) => Promise<boolean>;
	} = $props();

	let selectedCategoryId = $state('new');
	let activeTab = $state('categories');
	let categoryName = $state('');
	let categoryColor = $state(colorOptions[0]);
	let categoryOrderIndex = $state('0');
	let selectedTagId = $state('new');
	let tagName = $state('');
	let deleteDialogOpen = $state(false);
	let deleteCandidate = $state<DeleteCandidate | null>(null);

	let activeCategories = $derived(categories.filter((category) => category.isActive));
	let editingCategory = $derived(selectedCategoryId !== 'new');
	let editingTag = $derived(selectedTagId !== 'new');

	function selectCategory(category: CalendarCategory | null) {
		selectedCategoryId = category?.id ?? 'new';
		categoryName = category?.name ?? '';
		categoryColor =
			category && colorOptions.includes(category.color) ? category.color : colorOptions[0];
		categoryOrderIndex = category?.orderIndex.toString() ?? '0';
	}

	function selectTag(tag: CalendarTag | null) {
		selectedTagId = tag?.id ?? 'new';
		tagName = tag?.name ?? '';
	}

	async function saveCategory() {
		const normalizedOrder = Number(categoryOrderIndex);
		const saved = await onsavecategory?.(selectedCategoryId === 'new' ? null : selectedCategoryId, {
			name: categoryName.trim(),
			color: categoryColor,
			orderIndex: Number.isFinite(normalizedOrder) ? normalizedOrder : 0,
			isActive: true
		});
		if (saved && selectedCategoryId === 'new') selectCategory(null);
	}

	async function saveTag() {
		const saved = await onsavetag?.(selectedTagId === 'new' ? null : selectedTagId, {
			name: tagName.trim()
		});
		if (saved && selectedTagId === 'new') selectTag(null);
	}

	function requestDelete(candidate: DeleteCandidate) {
		deleteCandidate = candidate;
		deleteDialogOpen = true;
	}

	async function confirmDelete() {
		if (!deleteCandidate) return;

		const deleted =
			deleteCandidate.kind === 'category'
				? await ondeletecategory?.(deleteCandidate.item)
				: await ondeletetag?.(deleteCandidate.item);
		if (!deleted) return;

		if (deleteCandidate.kind === 'category') selectCategory(null);
		else selectTag(null);
		deleteCandidate = null;
		deleteDialogOpen = false;
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-3xl">
		<Dialog.Header>
			<Dialog.Title>จัดการหมวดหมู่และแท็ก</Dialog.Title>
			<Dialog.Description>
				ใช้หมวดหมู่หลักเพื่อกำหนดสีบนปฏิทิน และใช้แท็กได้หลายรายการต่อกิจกรรม
			</Dialog.Description>
		</Dialog.Header>

		<Tabs.Root bind:value={activeTab}>
			<Tabs.List class="grid w-full grid-cols-2">
				<Tabs.Trigger value="categories">หมวดหมู่หลัก</Tabs.Trigger>
				<Tabs.Trigger value="tags">แท็ก</Tabs.Trigger>
			</Tabs.List>

			<Tabs.Content value="categories" class="mt-5">
				<div class="grid gap-5 md:grid-cols-[220px_1fr]">
					<section class="space-y-2">
						<Button
							type="button"
							variant={selectedCategoryId === 'new' ? 'default' : 'outline'}
							class="w-full justify-start"
							onclick={() => selectCategory(null)}
						>
							<Plus class="size-4" />
							สร้างหมวดหมู่ใหม่
						</Button>
						<div class="max-h-72 space-y-1 overflow-y-auto rounded-md border p-2">
							{#each activeCategories as category (category.id)}
								<Button
									type="button"
									variant={selectedCategoryId === category.id ? 'secondary' : 'ghost'}
									class="w-full justify-start"
									onclick={() => selectCategory(category)}
								>
									<span
										class="size-3 shrink-0 rounded-full"
										style:background-color={category.color}
										aria-hidden="true"
									></span>
									<span class="truncate">{category.name}</span>
								</Button>
							{/each}
							{#if activeCategories.length === 0}
								<p class="px-2 py-6 text-center text-sm text-muted-foreground">ยังไม่มีหมวดหมู่</p>
							{/if}
						</div>
					</section>

					<form
						class="space-y-4"
						onsubmit={(event) => {
							event.preventDefault();
							void saveCategory();
						}}
					>
						<div class="grid gap-2">
							<Label for="calendar-category-name">ชื่อหมวดหมู่</Label>
							<Input
								id="calendar-category-name"
								bind:value={categoryName}
								required
								maxlength={120}
							/>
						</div>

						<div class="grid gap-2">
							<Label>สีบนปฏิทิน</Label>
							<div class="flex flex-wrap gap-2">
								{#each colorOptions as option (option)}
									<button
										type="button"
										class={cn(
											'size-9 rounded-md border shadow-xs outline-none transition focus-visible:ring-2 focus-visible:ring-ring',
											categoryColor === option && 'ring-2 ring-ring ring-offset-2'
										)}
										style:background-color={option}
										aria-label={`เลือกสี ${option}`}
										aria-pressed={categoryColor === option}
										onclick={() => (categoryColor = option)}
									></button>
								{/each}
							</div>
						</div>

						<div class="grid max-w-40 gap-2">
							<Label for="calendar-category-order">ลำดับ</Label>
							<Input
								id="calendar-category-order"
								type="number"
								step="1"
								min="0"
								bind:value={categoryOrderIndex}
							/>
						</div>

						<div class="flex flex-wrap justify-end gap-2 pt-2">
							{#if editingCategory}
								<Button
									type="button"
									variant="destructive"
									disabled={saving}
									onclick={() => {
										const category = activeCategories.find(
											(item) => item.id === selectedCategoryId
										);
										if (category) requestDelete({ kind: 'category', item: category });
									}}
								>
									<Trash2 class="size-4" />
									ลบถาวร
								</Button>
							{/if}
							<LoadingButton
								type="submit"
								loading={saving}
								loadingLabel="กำลังบันทึก..."
								disabled={!categoryName.trim()}
							>
								{editingCategory ? 'บันทึกหมวดหมู่' : 'สร้างหมวดหมู่'}
							</LoadingButton>
						</div>
					</form>
				</div>
			</Tabs.Content>

			<Tabs.Content value="tags" class="mt-5">
				<div class="grid gap-5 md:grid-cols-[220px_1fr]">
					<section class="space-y-2">
						<Button
							type="button"
							variant={selectedTagId === 'new' ? 'default' : 'outline'}
							class="w-full justify-start"
							onclick={() => selectTag(null)}
						>
							<Plus class="size-4" />
							สร้างแท็กใหม่
						</Button>
						<div class="max-h-72 space-y-1 overflow-y-auto rounded-md border p-2">
							{#each tags as tag (tag.id)}
								<Button
									type="button"
									variant={selectedTagId === tag.id ? 'secondary' : 'ghost'}
									class="w-full justify-start"
									onclick={() => selectTag(tag)}
								>
									<Tag class="size-4 shrink-0" />
									<span class="truncate">{tag.name}</span>
								</Button>
							{/each}
							{#if tags.length === 0}
								<p class="px-2 py-6 text-center text-sm text-muted-foreground">ยังไม่มีแท็ก</p>
							{/if}
						</div>
					</section>

					<form
						class="space-y-4"
						onsubmit={(event) => {
							event.preventDefault();
							void saveTag();
						}}
					>
						<div class="grid gap-2">
							<Label for="calendar-tag-name">ชื่อแท็ก</Label>
							<Input id="calendar-tag-name" bind:value={tagName} required maxlength={80} />
							<p class="text-xs text-muted-foreground">
								กิจกรรมหนึ่งรายการเลือกได้หลายแท็ก และใช้แท็กช่วยค้นหาหรือกรองได้
							</p>
						</div>

						<div class="flex flex-wrap justify-end gap-2 pt-2">
							{#if editingTag}
								<Button
									type="button"
									variant="destructive"
									disabled={saving}
									onclick={() => {
										const tag = tags.find((item) => item.id === selectedTagId);
										if (tag) requestDelete({ kind: 'tag', item: tag });
									}}
								>
									<Trash2 class="size-4" />
									ลบถาวร
								</Button>
							{/if}
							<LoadingButton
								type="submit"
								loading={saving}
								loadingLabel="กำลังบันทึก..."
								disabled={!tagName.trim()}
							>
								{editingTag ? 'บันทึกแท็ก' : 'สร้างแท็ก'}
							</LoadingButton>
						</div>
					</form>
				</div>
			</Tabs.Content>
		</Tabs.Root>

		<Dialog.Footer>
			<Button type="button" variant="outline" onclick={() => (open = false)}>ปิด</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<AlertDialog.Root bind:open={deleteDialogOpen}>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>
				ลบ{deleteCandidate?.kind === 'category' ? 'หมวดหมู่' : 'แท็ก'}นี้ถาวรหรือไม่
			</AlertDialog.Title>
			<AlertDialog.Description>
				{#if deleteCandidate?.kind === 'category'}
					กิจกรรมที่ใช้หมวดหมู่ “{deleteCandidate.item.name}” จะยังอยู่ครบ
					แต่จะแสดงเป็นไม่ระบุหมวดหมู่
				{:else}
					แท็ก “{deleteCandidate?.item.name ?? ''}” จะถูกนำออกจากทุกกิจกรรม โดยกิจกรรมจะไม่ถูกลบ
				{/if}
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel disabled={saving}>ยกเลิก</AlertDialog.Cancel>
			<AlertDialog.Action
				variant="destructive"
				disabled={saving}
				onclick={() => void confirmDelete()}
			>
				{saving ? 'กำลังลบ...' : 'ลบถาวร'}
			</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
