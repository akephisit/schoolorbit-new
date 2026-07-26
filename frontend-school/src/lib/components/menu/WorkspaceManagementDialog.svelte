<script lang="ts">
	import {
		createMenuWorkspace,
		deleteMenuWorkspace,
		updateMenuWorkspace,
		type MenuWorkspace
	} from '$lib/api/menu-admin';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Switch } from '$lib/components/ui/switch';
	import { LoaderCircle } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	interface Props {
		open: boolean;
		workspace: MenuWorkspace | null;
		canCreate?: boolean;
		canUpdate?: boolean;
		canDelete?: boolean;
		onSuccess: (
			result: { type: 'upsert'; workspace: MenuWorkspace } | { type: 'delete'; workspaceId: string }
		) => void;
		onOpenChange: (open: boolean) => void;
	}

	let {
		open = $bindable(),
		workspace,
		canCreate = false,
		canUpdate = false,
		canDelete = false,
		onSuccess,
		onOpenChange
	}: Props = $props();

	const protectedCodes = new Set(['home', 'operations', 'settings']);
	const canEdit = $derived(workspace ? canUpdate : canCreate);
	const canRemove = $derived(
		Boolean(workspace && canDelete && !protectedCodes.has(workspace.code))
	);

	let saving = $state(false);
	let formData = $state({
		code: '',
		name: '',
		name_en: '',
		icon: '',
		is_active: true
	});

	$effect(() => {
		if (!open) return;

		formData = workspace
			? {
					code: workspace.code,
					name: workspace.name,
					name_en: workspace.name_en ?? '',
					icon: workspace.icon ?? '',
					is_active: workspace.is_active
				}
			: {
					code: '',
					name: '',
					name_en: '',
					icon: '',
					is_active: true
				};
	});

	async function handleSubmit() {
		if (!canEdit || !formData.name.trim() || (!workspace && !formData.code.trim())) {
			toast.error('กรุณากรอกข้อมูลที่จำเป็น');
			return;
		}

		saving = true;
		try {
			const savedWorkspace = workspace
				? await updateMenuWorkspace(workspace.id, {
						name: formData.name.trim(),
						name_en: formData.name_en.trim() || null,
						icon: formData.icon.trim() || null,
						is_active: formData.is_active
					})
				: await createMenuWorkspace({
						code: formData.code.trim(),
						name: formData.name.trim(),
						name_en: formData.name_en.trim() || null,
						icon: formData.icon.trim() || null
					});

			toast.success(workspace ? 'แก้ไขกลุ่มบริหารสำเร็จ' : 'สร้างกลุ่มบริหารสำเร็จ');
			onSuccess({ type: 'upsert', workspace: savedWorkspace });
			open = false;
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถบันทึกกลุ่มบริหารได้');
		} finally {
			saving = false;
		}
	}

	async function handleDelete() {
		if (!workspace || !canRemove) return;
		if (
			!confirm(
				`ต้องการลบ "${workspace.name}" ใช่หรือไม่?\n\nฝ่าย/งานในกลุ่มนี้จะถูกย้ายไปยังกลุ่มบริหารทั่วไป`
			)
		) {
			return;
		}

		saving = true;
		try {
			await deleteMenuWorkspace(workspace.id);
			toast.success('ลบกลุ่มบริหารสำเร็จ');
			onSuccess({ type: 'delete', workspaceId: workspace.id });
			open = false;
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถลบกลุ่มบริหารได้');
		} finally {
			saving = false;
		}
	}
</script>

<Dialog.Root bind:open {onOpenChange}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>{workspace ? 'แก้ไขกลุ่มบริหาร' : 'สร้างกลุ่มบริหารใหม่'}</Dialog.Title>
			<Dialog.Description>
				กลุ่มบริหารเป็นหมวดระดับบนสุด เช่น วิชาการ งานบุคคล หรืองบประมาณ
			</Dialog.Description>
		</Dialog.Header>

		<form
			class="space-y-4"
			onsubmit={(event) => {
				event.preventDefault();
				void handleSubmit();
			}}
		>
			{#if !workspace}
				<div class="space-y-2">
					<Label for="workspace-code">รหัส *</Label>
					<Input
						id="workspace-code"
						bind:value={formData.code}
						placeholder="เช่น budget"
						required
						disabled={saving || !canEdit}
					/>
					<p class="text-xs text-muted-foreground">ใช้ตัวอักษรอังกฤษ ตัวเลข _ หรือ -</p>
				</div>
			{/if}

			<div class="space-y-2">
				<Label for="workspace-name">ชื่อกลุ่มบริหาร *</Label>
				<Input
					id="workspace-name"
					bind:value={formData.name}
					placeholder="เช่น กลุ่มบริหารงบประมาณ"
					required
					disabled={saving || !canEdit}
				/>
			</div>

			<div class="space-y-2">
				<Label for="workspace-name-en">ชื่อภาษาอังกฤษ</Label>
				<Input
					id="workspace-name-en"
					bind:value={formData.name_en}
					placeholder="Budget Administration"
					disabled={saving || !canEdit}
				/>
			</div>

			<div class="space-y-2">
				<Label for="workspace-icon">Icon</Label>
				<Input
					id="workspace-icon"
					bind:value={formData.icon}
					placeholder="เช่น WalletCards"
					disabled={saving || !canEdit}
				/>
				<p class="text-xs text-muted-foreground">ใช้ชื่อไอคอนจาก Lucide</p>
			</div>

			{#if workspace}
				<div class="flex items-center justify-between rounded-lg border p-3">
					<div>
						<Label for="workspace-active">เปิดใช้งาน</Label>
						<p class="text-xs text-muted-foreground">เมื่อปิด หมวดนี้จะไม่ปรากฏในเมนูผู้ใช้</p>
					</div>
					<Switch
						id="workspace-active"
						bind:checked={formData.is_active}
						disabled={saving || !canEdit}
					/>
				</div>
			{/if}

			<Dialog.Footer class="gap-2 sm:justify-between">
				<div>
					{#if canRemove}
						<Button type="button" variant="destructive" onclick={handleDelete} disabled={saving}>
							ลบ
						</Button>
					{/if}
				</div>
				<div class="flex gap-2">
					<Button type="button" variant="outline" onclick={() => (open = false)} disabled={saving}>
						ยกเลิก
					</Button>
					{#if canEdit}
						<Button type="submit" disabled={saving}>
							{#if saving}
								<LoaderCircle class="mr-2 h-4 w-4 animate-spin" />
							{/if}
							บันทึก
						</Button>
					{/if}
				</div>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
