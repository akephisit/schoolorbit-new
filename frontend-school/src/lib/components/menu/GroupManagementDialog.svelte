<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { appIdentityKey } from '#lib/auth/settled-user.js';
	let disposed = false;
	onDestroy(() => {
		disposed = true;
	});
	import type { MenuGroup, MenuWorkspace } from '#lib/api/menu-admin.js';
	import { createMenuGroup, updateMenuGroup, deleteMenuGroup } from '#lib/api/menu-admin.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { toast } from 'svelte-sonner';
	import { LoaderCircle } from '@lucide/svelte';

	interface Props {
		open: boolean;
		group: MenuGroup | null; // null = create mode
		workspaces: MenuWorkspace[];
		canCreate?: boolean;
		canUpdate?: boolean;
		canDelete?: boolean;
		onSuccess: (
			result: { type: 'upsert'; group: MenuGroup } | { type: 'delete'; groupId: string }
		) => void;
		onOpenChange: (open: boolean) => void;
	}

	let {
		open = $bindable(),
		group,
		workspaces,
		canCreate = false,
		canUpdate = false,
		canDelete = false,
		onSuccess,
		onOpenChange
	}: Props = $props();

	let saving = $state(false);
	let formData = $state({
		code: '',
		name: '',
		name_en: '',
		icon: '',
		workspace_code: ''
	});

	const canEditGroup = $derived(group ? canUpdate : canCreate);
	const selectedWorkspaceName = $derived(
		workspaces.find((workspace) => workspace.code === formData.workspace_code)?.name ??
			'เลือกกลุ่มบริหาร'
	);

	// Reset form when dialog opens/closes or group changes
	$effect.pre(() => {
		const opened = open,
			target = group?.id;
		untrack(() => {
			void target;
			void opened;
			if (open && group) {
				// Edit mode
				formData = {
					code: group.code,
					name: group.name,
					name_en: group.name_en || '',
					icon: group.icon || '',
					workspace_code: group.workspace_code
				};
			} else if (open && !group) {
				// Create mode
				formData = {
					code: '',
					name: '',
					name_en: '',
					icon: '',
					workspace_code: workspaces.find((workspace) => workspace.is_active)?.code ?? ''
				};
			}
		});
	});

	async function handleSubmit() {
		if (disposed || !open || saving || !canEditGroup) {
			toast.error('ไม่มีสิทธิ์บันทึกกลุ่มเมนู');
			return;
		}

		if (!formData.name || !formData.workspace_code || (!group && !formData.code)) {
			toast.error('กรุณากรอกข้อมูลที่จำเป็น');
			return;
		}

		const target = group,
			identity = appIdentityKey();
		const current = () => identity === appIdentityKey() && (target ? canUpdate : canCreate);
		const ownsDraft = () => current() && !disposed && open;
		saving = true;
		try {
			let savedGroup: MenuGroup;
			if (target) {
				// Update
				savedGroup = await updateMenuGroup(target.id, {
					name: formData.name,
					name_en: formData.name_en || undefined,
					icon: formData.icon || undefined,
					workspace_code: formData.workspace_code
				});
			} else {
				// Create
				savedGroup = await createMenuGroup({
					code: formData.code,
					name: formData.name,
					name_en: formData.name_en || undefined,
					icon: formData.icon || undefined,
					workspace_code: formData.workspace_code
				});
			}
			if (current()) onSuccess({ type: 'upsert', group: savedGroup });
			if (ownsDraft()) {
				toast.success(target ? 'แก้ไขกลุ่มเมนูสำเร็จ' : 'สร้างกลุ่มเมนูสำเร็จ');
				open = false;
			}
		} catch (error) {
			const message = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
			if (ownsDraft()) toast.error(message);
		} finally {
			if (ownsDraft()) saving = false;
		}
	}

	async function handleDelete() {
		if (disposed || !open || saving || !group) return;
		if (!canDelete) {
			toast.error('ไม่มีสิทธิ์ลบกลุ่มเมนู');
			return;
		}

		if (group.code === 'other') {
			toast.error('ไม่สามารถลบกลุ่ม "อื่นๆ" ได้');
			return;
		}

		if (
			!confirm(
				`ต้องการลบกลุ่ม "${group.name}" ใช่หรือไม่?\n\nรายการเมนูในกลุ่มนี้จะถูกย้ายไปยัง "อื่นๆ"`
			)
		) {
			return;
		}

		const target = group,
			identity = appIdentityKey();
		const current = () => identity === appIdentityKey() && canDelete;
		const ownsDraft = () => current() && !disposed && open;
		saving = true;
		try {
			await deleteMenuGroup(target.id);
			if (current()) onSuccess({ type: 'delete', groupId: target.id });
			if (ownsDraft()) {
				toast.success('ลบกลุ่มเมนูสำเร็จ');
				open = false;
			}
		} catch (error) {
			const message = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
			if (ownsDraft()) toast.error(message);
		} finally {
			if (ownsDraft()) saving = false;
		}
	}
</script>

<Dialog.Root bind:open {onOpenChange}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>{group ? 'แก้ไขกลุ่มเมนู' : 'สร้างกลุ่มเมนูใหม่'}</Dialog.Title>
			<Dialog.Description>
				{group ? 'แก้ไขข้อมูลกลุ่มเมนู' : 'เพิ่มกลุ่มเมนูใหม่สำหรับจัดระเบียบ'}
			</Dialog.Description>
		</Dialog.Header>

		<form
			onsubmit={(e) => {
				e.preventDefault();
				handleSubmit();
			}}
			class="space-y-4"
		>
			{#if !group}
				<!-- Code (create only) -->
				<div class="space-y-2">
					<Label for="code">รหัส (Code) *</Label>
					<Input
						id="code"
						bind:value={formData.code}
						placeholder="เช่น reports, finance"
						required
						disabled={saving || !canEditGroup}
					/>
					<p class="text-xs text-muted-foreground">ใช้ตัวอักษรภาษาอังกฤษและ - เท่านั้น</p>
				</div>
			{/if}

			<div class="space-y-2">
				<Label for="workspace">กลุ่มบริหาร *</Label>
				<Select.Root
					type="single"
					bind:value={formData.workspace_code}
					disabled={saving || !canEditGroup}
				>
					<Select.Trigger id="workspace" class="w-full">{selectedWorkspaceName}</Select.Trigger>
					<Select.Content>
						{#each workspaces.filter((workspace) => workspace.is_active) as workspace (workspace.id)}
							<Select.Item value={workspace.code}>{workspace.name}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
				<p class="text-xs text-muted-foreground">ฝ่าย/งานนี้จะแสดงอยู่ใต้กลุ่มบริหารที่เลือก</p>
			</div>

			<!-- Name -->
			<div class="space-y-2">
				<Label for="name">ชื่อกลุ่ม (ไทย) *</Label>
				<Input
					id="name"
					bind:value={formData.name}
					placeholder="เช่น รายงาน, การเงิน"
					required
					disabled={saving || !canEditGroup}
				/>
			</div>

			<!-- Name EN -->
			<div class="space-y-2">
				<Label for="name_en">ชื่อกลุ่ม (English)</Label>
				<Input
					id="name_en"
					bind:value={formData.name_en}
					placeholder="e.g. Reports, Finance"
					disabled={saving || !canEditGroup}
				/>
			</div>

			<!-- Icon -->
			<div class="space-y-2">
				<Label for="icon">Icon</Label>
				<Input
					id="icon"
					bind:value={formData.icon}
					placeholder="เช่น chart-bar, wallet"
					disabled={saving || !canEditGroup}
				/>
				<p class="text-xs text-muted-foreground">ใช้ชื่อ icon จาก Lucide Icons</p>
			</div>

			<Dialog.Footer class="flex-col sm:flex-row gap-2">
				<div class="flex-1">
					{#if group && group.code !== 'other' && canDelete}
						<Button
							type="button"
							variant="destructive"
							onclick={handleDelete}
							disabled={saving}
							class="w-full sm:w-auto"
						>
							{#if saving}
								<LoaderCircle class="h-4 w-4 animate-spin mr-2" />
							{/if}
							ลบ
						</Button>
					{/if}
				</div>
				<div class="flex gap-2">
					<Button type="button" variant="outline" onclick={() => (open = false)} disabled={saving}>
						ยกเลิก
					</Button>
					{#if canEditGroup}
						<Button type="submit" disabled={saving}>
							{#if saving}
								<LoaderCircle class="h-4 w-4 animate-spin mr-2" />
							{/if}
							{group ? 'บันทึก' : 'สร้าง'}
						</Button>
					{/if}
				</div>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
