<script lang="ts">
	import type { MenuGroup } from '$lib/api/menu-admin';
	import { createMenuGroup, updateMenuGroup, deleteMenuGroup } from '$lib/api/menu-admin';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { toast } from 'svelte-sonner';
	import { LoaderCircle } from 'lucide-svelte';

	interface Props {
		open: boolean;
		group: MenuGroup | null; // null = create mode
		onSuccess: () => void;
		onOpenChange: (open: boolean) => void;
	}

	let { open = $bindable(), group, onSuccess, onOpenChange }: Props = $props();

	let saving = $state(false);
	let formData = $state({
		code: '',
		name: '',
		name_en: '',
		icon: ''
	});

	// Reset form when dialog opens/closes or group changes
	$effect(() => {
		if (open && group) {
			// Edit mode
			formData = {
				code: group.code,
				name: group.name,
				name_en: group.name_en || '',
				icon: group.icon || ''
			};
		} else if (open && !group) {
			// Create mode
			formData = {
				code: '',
				name: '',
				name_en: '',
				icon: ''
			};
		}
	});

	async function handleSubmit() {
		if (!formData.name || (!group && !formData.code)) {
			toast.error('กรุณากรอกข้อมูลที่จำเป็น');
			return;
		}

		saving = true;
		try {
			if (group) {
				// Update
				await updateMenuGroup(group.id, {
					name: formData.name,
					name_en: formData.name_en || undefined,
					icon: formData.icon || undefined
				});
				toast.success('แก้ไขกลุ่มเมนูสำเร็จ');
			} else {
				// Create
				await createMenuGroup({
					code: formData.code,
					name: formData.name,
					name_en: formData.name_en || undefined,
					icon: formData.icon || undefined
				});
				toast.success('สร้างกลุ่มเมนูสำเร็จ');
			}
			onSuccess();
			open = false;
		} catch (error) {
			const message = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
			toast.error(message);
		} finally {
			saving = false;
		}
	}

	async function handleDelete() {
		if (!group) return;

		if (group.code === 'other') {
			toast.error('ไม่สามารถลบกลุ่ม "อื่นๆ" ได้');
			return;
		}

		if (!confirm(`ต้องการลบกลุ่ม "${group.name}" ใช่หรือไม่?\n\nรายการเมนูในกลุ่มนี้จะถูกย้ายไปยัง "อื่นๆ"`)) {
			return;
		}

		saving = true;
		try {
			await deleteMenuGroup(group.id);
			toast.success('ลบกลุ่มเมนูสำเร็จ');
			onSuccess();
			open = false;
		} catch (error) {
			const message = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
			toast.error(message);
		} finally {
			saving = false;
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
						disabled={saving}
					/>
					<p class="text-xs text-muted-foreground">ใช้ตัวอักษรภาษาอังกฤษและ - เท่านั้น</p>
				</div>
			{/if}

			<!-- Name -->
			<div class="space-y-2">
				<Label for="name">ชื่อกลุ่ม (ไทย) *</Label>
				<Input
					id="name"
					bind:value={formData.name}
					placeholder="เช่น รายงาน, การเงิน"
					required
					disabled={saving}
				/>
			</div>

			<!-- Name EN -->
			<div class="space-y-2">
				<Label for="name_en">ชื่อกลุ่ม (English)</Label>
				<Input
					id="name_en"
					bind:value={formData.name_en}
					placeholder="e.g. Reports, Finance"
					disabled={saving}
				/>
			</div>

			<!-- Icon -->
			<div class="space-y-2">
				<Label for="icon">Icon</Label>
				<Input
					id="icon"
					bind:value={formData.icon}
					placeholder="เช่น chart-bar, wallet"
					disabled={saving}
				/>
				<p class="text-xs text-muted-foreground">ใช้ชื่อ icon จาก Lucide Icons</p>
			</div>

			<Dialog.Footer class="flex-col sm:flex-row gap-2">
				<div class="flex-1">
					{#if group && group.code !== 'other'}
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
					<Button type="submit" disabled={saving}>
						{#if saving}
							<LoaderCircle class="h-4 w-4 animate-spin mr-2" />
						{/if}
						{group ? 'บันทึก' : 'สร้าง'}
					</Button>
				</div>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
