<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { LoadingButton } from '$lib/components/app-state';
	import type { CalendarCategory, UpsertCalendarCategoryRequest } from '$lib/api/calendar';
	import { cn } from '$lib/utils';

	const colorOptions = ['#2563eb', '#16a34a', '#f59e0b', '#dc2626', '#7c3aed', '#0891b2'];

	let {
		open = $bindable(false),
		categories = [],
		saving = false,
		onsave,
		ondeactivate
	}: {
		open: boolean;
		categories?: CalendarCategory[];
		saving?: boolean;
		onsave?: (id: string | null, payload: UpsertCalendarCategoryRequest) => void;
		ondeactivate?: (category: CalendarCategory) => void;
	} = $props();

	let selectedCategoryId = $state('new');
	let name = $state('');
	let color = $state(colorOptions[0]);
	let orderIndex = $state('0');
	let loadedSelection = $state('');

	let activeCategories = $derived(categories.filter((category) => category.isActive));
	let selectedCategory = $derived(
		activeCategories.find((category) => category.id === selectedCategoryId) ?? null
	);
	let editing = $derived(selectedCategory !== null);

	$effect(() => {
		if (!open) {
			loadedSelection = '';
			return;
		}

		if (selectedCategoryId !== 'new' && !selectedCategory) {
			selectedCategoryId = 'new';
		}

		if (loadedSelection !== selectedCategoryId) {
			loadSelectedCategory();
			loadedSelection = selectedCategoryId;
		}
	});

	function loadSelectedCategory() {
		if (!selectedCategory) {
			name = '';
			color = colorOptions[0];
			orderIndex = '0';
			return;
		}

		name = selectedCategory.name;
		color = colorOptions.includes(selectedCategory.color)
			? selectedCategory.color
			: colorOptions[0];
		orderIndex = selectedCategory.orderIndex.toString();
	}

	function saveCategory() {
		const normalizedOrder = Number(orderIndex);
		onsave?.(selectedCategory?.id ?? null, {
			name: name.trim(),
			color,
			orderIndex: Number.isFinite(normalizedOrder) ? normalizedOrder : 0,
			isActive: true
		});
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-2xl">
		<Dialog.Header>
			<Dialog.Title>หมวดหมู่ปฏิทิน</Dialog.Title>
			<Dialog.Description>สร้าง แก้ไข และปิดใช้งานหมวดหมู่กิจกรรม</Dialog.Description>
		</Dialog.Header>

		<div class="grid gap-5 md:grid-cols-[220px_1fr]">
			<section class="space-y-2">
				<Button
					type="button"
					variant={selectedCategoryId === 'new' ? 'default' : 'outline'}
					class="w-full justify-start"
					onclick={() => (selectedCategoryId = 'new')}
				>
					สร้างหมวดหมู่ใหม่
				</Button>
				<div class="max-h-72 space-y-1 overflow-y-auto rounded-md border p-2">
					{#each activeCategories as category (category.id)}
						<Button
							type="button"
							variant={selectedCategoryId === category.id ? 'secondary' : 'ghost'}
							class="w-full justify-start"
							onclick={() => (selectedCategoryId = category.id)}
						>
							<span
								class="size-3 shrink-0 rounded-full"
								style={`background-color: ${category.color}`}
								aria-hidden="true"
							></span>
							<span class="truncate">{category.name}</span>
						</Button>
					{/each}
					{#if activeCategories.length === 0}
						<p class="px-2 py-6 text-center text-sm text-muted-foreground">
							ยังไม่มีหมวดหมู่ที่ใช้งาน
						</p>
					{/if}
				</div>
			</section>

			<form
				class="space-y-4"
				onsubmit={(submitEvent) => {
					submitEvent.preventDefault();
					saveCategory();
				}}
			>
				<div class="grid gap-2">
					<Label for="calendar-category-name">ชื่อหมวดหมู่</Label>
					<Input id="calendar-category-name" bind:value={name} required maxlength={120} />
				</div>

				<div class="grid gap-2">
					<Label>สี</Label>
					<div class="flex flex-wrap gap-2">
						{#each colorOptions as option (option)}
							<button
								type="button"
								class={cn(
									'size-9 rounded-md border shadow-xs outline-none transition focus-visible:ring-2 focus-visible:ring-ring',
									color === option && 'ring-2 ring-ring ring-offset-2'
								)}
								style={`background-color: ${option}`}
								aria-label={`เลือกสี ${option}`}
								aria-pressed={color === option}
								onclick={() => (color = option)}
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
						bind:value={orderIndex}
					/>
				</div>

				<Dialog.Footer class="pt-2">
					{#if editing && selectedCategory}
						<Button
							type="button"
							variant="destructive"
							onclick={() => ondeactivate?.(selectedCategory)}
						>
							ปิดใช้งาน
						</Button>
					{/if}
					<Button type="button" variant="outline" onclick={() => (open = false)}>ยกเลิก</Button>
					<LoadingButton
						type="submit"
						loading={saving}
						loadingLabel="กำลังบันทึก..."
						disabled={!name.trim()}
					>
						{editing ? 'บันทึกหมวดหมู่' : 'สร้างหมวดหมู่'}
					</LoadingButton>
				</Dialog.Footer>
			</form>
		</div>
	</Dialog.Content>
</Dialog.Root>
