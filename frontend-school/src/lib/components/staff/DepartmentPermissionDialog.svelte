<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import {
		listMenuGroups,
		listMenuItems,
		type MenuGroup,
		type MenuItem
	} from '$lib/api/menu-admin';
	import { getDepartmentMenus, updateDepartmentMenus, type Department } from '$lib/api/staff';
	import { toast } from 'svelte-sonner';
	import { LoaderCircle, Folder, File } from 'lucide-svelte';

	let {
		open = $bindable(false),
		department,
		onSuccess
	} = $props<{
		department: Department | null;
		open: boolean;
		onSuccess?: () => void;
	}>();

	let groups = $state<MenuGroup[]>([]);
	let items = $state<MenuItem[]>([]);
	let selectedMenuIds = $state<Set<string>>(new Set());
	let loading = $state(false);
	let saving = $state(false);

	$effect(() => {
		if (open && department) {
			loadData();
		}
	});

	async function loadData() {
		try {
			loading = true;
			// Load groups, items, and current permissions
			const [g, i, currentAccess] = await Promise.all([
				listMenuGroups(),
				listMenuItems(),
				getDepartmentMenus(department!.id)
			]);

			groups = g.sort((a, b) => a.display_order - b.display_order);
			items = i.sort((a, b) => a.display_order - b.display_order);
			selectedMenuIds = new Set(currentAccess);
		} catch (e) {
			toast.error('โหลดข้อมูลเมนูไม่สำเร็จ');
			console.error(e);
		} finally {
			loading = false;
		}
	}

	async function handleSave() {
		if (!department) return;
		try {
			saving = true;
			await updateDepartmentMenus(department.id, Array.from(selectedMenuIds));
			toast.success('บันทึกสิทธิ์การเข้าถึงเมนูสำเร็จ');
			open = false;
			onSuccess?.();
		} catch (e) {
			toast.error('บันทึกไม่สำเร็จ');
		} finally {
			saving = false;
		}
	}

	function toggleGroup(groupId: string, select: boolean) {
		const itemsInGroup = items.filter((i) => i.group_id === groupId);
		const newSet = new Set(selectedMenuIds);

		itemsInGroup.forEach((item) => {
			if (select) newSet.add(item.id);
			else newSet.delete(item.id);
		});

		selectedMenuIds = newSet;
	}

	function isGroupSelected(groupId: string): boolean {
		const itemsInGroup = items.filter((i) => i.group_id === groupId);
		if (itemsInGroup.length === 0) return false;
		return itemsInGroup.every((i) => selectedMenuIds.has(i.id));
	}

	function isGroupIndeterminate(groupId: string): boolean {
		const itemsInGroup = items.filter((i) => i.group_id === groupId);
		if (itemsInGroup.length === 0) return false;
		const selectedCount = itemsInGroup.filter((i) => selectedMenuIds.has(i.id)).length;
		return selectedCount > 0 && selectedCount < itemsInGroup.length;
	}

	function toggleItem(itemId: string, checked: boolean) {
		const newSet = new Set(selectedMenuIds);
		if (checked) newSet.add(itemId);
		else newSet.delete(itemId);
		selectedMenuIds = newSet;
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-[700px] max-h-[85vh] flex flex-col p-6">
		<Dialog.Header>
			<Dialog.Title class="text-xl">กำหนดสิทธิ์การเข้าถึงเมนู</Dialog.Title>
			<Dialog.Description>
				เลือกเมนูระบบที่ฝ่าย <span class="font-bold text-foreground">{department?.name}</span> สามารถเข้าถึงได้
				(จะมีผลกับบุคลากรทุกคนในฝ่ายนี้)
			</Dialog.Description>
		</Dialog.Header>

		<div class="flex-1 overflow-y-auto py-4 pr-2 -mr-2">
			{#if loading}
				<div class="flex justify-center items-center py-20">
					<LoaderCircle class="w-8 h-8 animate-spin text-primary" />
				</div>
			{:else}
				<div class="space-y-6">
					{#each groups as group}
						<div class="border rounded-lg p-4 bg-muted/20">
							<div class="flex items-center gap-2 mb-3 pb-2 border-b border-border/50">
								<Checkbox
									checked={isGroupSelected(group.id)}
									indeterminate={isGroupIndeterminate(group.id)}
									onCheckedChange={(c) => toggleGroup(group.id, !!c)}
								/>
								<Folder class="w-4 h-4 text-primary" />
								<span class="font-semibold">{group.name}</span>
								{#if group.name_en}
									<span class="text-xs text-muted-foreground">({group.name_en})</span>
								{/if}
							</div>

							<div class="grid grid-cols-1 sm:grid-cols-2 gap-3 pl-7">
								{#each items.filter((i) => i.group_id === group.id) as item}
									<div
										class="flex items-center gap-2 p-2 rounded-md hover:bg-background border border-transparent hover:border-border transition-colors cursor-pointer"
										onclick={() => toggleItem(item.id, !selectedMenuIds.has(item.id))}
									>
										<Checkbox
											checked={selectedMenuIds.has(item.id)}
											onCheckedChange={(c) => toggleItem(item.id, !!c)}
											onclick={(e) => e.stopPropagation()}
										/>
										<File class="w-3.5 h-3.5 text-muted-foreground" />
										<span class="text-sm">{item.name}</span>
									</div>
								{/each}
								{#if items.filter((i) => i.group_id === group.id).length === 0}
									<div class="text-xs text-muted-foreground italic col-span-full">
										ไม่มีเมนูในกลุ่มนี้
									</div>
								{/if}
							</div>
						</div>
					{/each}

					{#if groups.length === 0}
						<div class="text-center py-10 text-muted-foreground">ไม่พบข้อมูลกลุ่มเมนู</div>
					{/if}
				</div>
			{/if}
		</div>

		<Dialog.Footer class="pt-4 border-t mt-2">
			<Button variant="outline" onclick={() => (open = false)}>ยกเลิก</Button>
			<Button onclick={handleSave} disabled={saving}>
				{#if saving}
					<LoaderCircle class="w-4 h-4 mr-2 animate-spin" />
				{/if}
				บันทึก
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
