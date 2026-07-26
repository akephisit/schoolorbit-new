<script lang="ts">
	import {
		updateMenuItem,
		type MenuGroup,
		type MenuItem,
		type MenuWorkspace
	} from '$lib/api/menu-admin';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Switch } from '$lib/components/ui/switch';
	import { LoaderCircle } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	interface Props {
		open: boolean;
		item: MenuItem | null;
		groups: MenuGroup[];
		workspaces: MenuWorkspace[];
		canUpdate?: boolean;
		onSuccess: (item: MenuItem) => void;
		onOpenChange: (open: boolean) => void;
	}

	let {
		open = $bindable(),
		item,
		groups,
		workspaces,
		canUpdate = false,
		onSuccess,
		onOpenChange
	}: Props = $props();

	let saving = $state(false);
	let formData = $state({
		name: '',
		name_en: '',
		icon: '',
		group_id: '',
		is_active: true
	});

	function groupLabel(group: MenuGroup): string {
		const workspaceName =
			workspaces.find((workspace) => workspace.code === group.workspace_code)?.name ??
			group.workspace_code;
		return `${workspaceName} / ${group.name}`;
	}

	const selectedGroup = $derived.by(() => {
		const group = groups.find((candidate) => candidate.id === formData.group_id);
		return group ? groupLabel(group) : 'เลือกกลุ่มบริหาร / ฝ่ายงาน';
	});

	$effect(() => {
		if (!open || !item) return;
		formData = {
			name: item.name,
			name_en: item.name_en ?? '',
			icon: item.icon ?? '',
			group_id: item.group_id ?? '',
			is_active: item.is_active
		};
	});

	async function handleSubmit() {
		if (!item || !canUpdate || !formData.name.trim() || !formData.group_id) {
			toast.error('กรุณากรอกข้อมูลที่จำเป็น');
			return;
		}

		saving = true;
		try {
			const savedItem = await updateMenuItem(item.id, {
				name: formData.name.trim(),
				name_en: formData.name_en.trim() || null,
				icon: formData.icon.trim() || null,
				group_id: formData.group_id,
				is_active: formData.is_active
			});
			toast.success('แก้ไขเมนูสำเร็จ');
			onSuccess(savedItem);
			open = false;
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถบันทึกเมนูได้');
		} finally {
			saving = false;
		}
	}
</script>

<Dialog.Root bind:open {onOpenChange}>
	<Dialog.Content class="sm:max-w-lg">
		<Dialog.Header>
			<Dialog.Title>แก้ไขเมนูบริการ</Dialog.Title>
			<Dialog.Description>
				เลือกฝ่าย/งานที่รับผิดชอบเมนูนี้ การย้ายตำแหน่งไม่มีผลต่อสิทธิ์ของผู้ใช้
			</Dialog.Description>
		</Dialog.Header>

		{#if item}
			<form
				class="space-y-4"
				onsubmit={(event) => {
					event.preventDefault();
					void handleSubmit();
				}}
			>
				<div class="flex flex-wrap items-center gap-2 rounded-lg bg-muted/50 p-3 text-sm">
					<code>{item.path}</code>
					{#if item.required_permission}
						<Badge variant="outline">{item.required_permission}</Badge>
					{/if}
				</div>

				<div class="space-y-2">
					<Label for="menu-item-name">ชื่อเมนู *</Label>
					<Input
						id="menu-item-name"
						bind:value={formData.name}
						required
						disabled={saving || !canUpdate}
					/>
				</div>

				<div class="space-y-2">
					<Label for="menu-item-name-en">ชื่อภาษาอังกฤษ</Label>
					<Input
						id="menu-item-name-en"
						bind:value={formData.name_en}
						disabled={saving || !canUpdate}
					/>
				</div>

				<div class="space-y-2">
					<Label for="menu-item-group">ฝ่าย/งาน *</Label>
					<Select.Root type="single" bind:value={formData.group_id} disabled={saving || !canUpdate}>
						<Select.Trigger id="menu-item-group" class="w-full">{selectedGroup}</Select.Trigger>
						<Select.Content>
							{#each groups.filter((group) => group.is_active) as group (group.id)}
								<Select.Item value={group.id}>{groupLabel(group)}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="space-y-2">
					<Label for="menu-item-icon">Icon</Label>
					<Input
						id="menu-item-icon"
						bind:value={formData.icon}
						placeholder="เช่น BookOpen"
						disabled={saving || !canUpdate}
					/>
				</div>

				<div class="flex items-center justify-between rounded-lg border p-3">
					<div>
						<Label for="menu-item-active">เปิดใช้งาน</Label>
						<p class="text-xs text-muted-foreground">เมนูที่ปิดจะไม่แสดงแก่ผู้ใช้ทุกคน</p>
					</div>
					<Switch
						id="menu-item-active"
						bind:checked={formData.is_active}
						disabled={saving || !canUpdate}
					/>
				</div>

				<Dialog.Footer>
					<Button type="button" variant="outline" onclick={() => (open = false)} disabled={saving}>
						ยกเลิก
					</Button>
					<Button type="submit" disabled={saving || !canUpdate}>
						{#if saving}
							<LoaderCircle class="mr-2 h-4 w-4 animate-spin" />
						{/if}
						บันทึก
					</Button>
				</Dialog.Footer>
			</form>
		{/if}
	</Dialog.Content>
</Dialog.Root>
