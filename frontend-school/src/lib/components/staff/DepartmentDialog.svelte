<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Textarea } from '$lib/components/ui/textarea';
	import { createDepartment, updateDepartment, type Department } from '$lib/api/staff';
	import { toast } from 'svelte-sonner';

	let {
		open = $bindable(false),
		departmentToEdit = null,
		departments = [],
		onSuccess,
		forcedCategory = undefined
	} = $props<{
		open: boolean;
		departmentToEdit?: Department | null;
		departments: Department[]; // For parent selection
		onSuccess: () => void;
		forcedCategory?: string; // Optional: If set, locks the category
	}>();

	let loading = $state(false);

	let formData = $state({
		code: '',
		name: '',
		name_en: '',
		description: '',
		parent_department_id: 'none',
		category: forcedCategory || 'administrative',
		org_type: 'unit',
		phone: '',
		email: '',
		location: '',
		display_order: 0
	});

	// Pre-fill data when departmentToEdit changes
	$effect(() => {
		if (departmentToEdit) {
			formData = {
				code: departmentToEdit.code,
				name: departmentToEdit.name,
				name_en: departmentToEdit.name_en || '',
				description: departmentToEdit.description || '',
				parent_department_id: departmentToEdit.parent_department_id || 'none',
				category: departmentToEdit.category || forcedCategory || 'administrative',
				org_type: departmentToEdit.org_type || 'unit',
				phone: departmentToEdit.phone || '',
				email: departmentToEdit.email || '',
				location: departmentToEdit.location || '',
				display_order: departmentToEdit.display_order || 0
			};
		} else {
			// Reset for create mode
			formData = {
				code: '',
				name: '',
				name_en: '',
				description: '',
				parent_department_id: 'none',
				category: forcedCategory || 'administrative',
				org_type: 'unit',
				phone: '',
				email: '',
				location: '',
				display_order: 0
			};
		}
	});

	async function handleSubmit(e: SubmitEvent) {
		e.preventDefault();
		loading = true;

		try {
			const payload: any = { ...formData };
			if (payload.parent_department_id === 'none') {
				delete payload.parent_department_id;
			}

			// Convert types if needed (e.g. string -> number)
			payload.display_order = Number(payload.display_order);

			if (departmentToEdit) {
				await updateDepartment(departmentToEdit.id, payload);
				toast.success('อัปเดตฝ่ายสำเร็จ');
			} else {
				await createDepartment(payload);
				toast.success('สร้างฝ่ายสำเร็จ');
			}
			open = false;
			onSuccess?.();
		} catch (error: any) {
			toast.error(error.message || 'เกิดข้อผิดพลาด');
		} finally {
			loading = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-[600px] max-h-[90vh] overflow-y-auto">
		<Dialog.Header>
			<Dialog.Title>{departmentToEdit ? 'แก้ไขฝ่าย' : 'สร้างฝ่ายใหม่'}</Dialog.Title>
			<Dialog.Description>กรอกข้อมูลรายละเอียดของฝ่าย/กลุ่มสาระ/หน่วยงาน</Dialog.Description>
		</Dialog.Header>

		<form onsubmit={handleSubmit} class="space-y-4 py-4">
			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-2">
					<Label for="code">รหัสฝ่าย *</Label>
					<Input
						id="code"
						bind:value={formData.code}
						placeholder="เช่น ACADEMIC"
						required
						disabled={!!departmentToEdit}
					/>
				</div>
				<div class="space-y-2">
					<Label for="display_order">ลำดับการแสดงผล</Label>
					<Input type="number" id="display_order" bind:value={formData.display_order} />
				</div>
			</div>

			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-2">
					<Label for="name">ชื่อฝ่าย (ไทย) *</Label>
					<Input
						id="name"
						bind:value={formData.name}
						placeholder="เช่น กลุ่มบริหารวิชาการ"
						required
					/>
				</div>
				<div class="space-y-2">
					<Label for="name_en">ชื่อฝ่าย (อังกฤษ)</Label>
					<Input id="name_en" bind:value={formData.name_en} placeholder="e.g. Academic Affairs" />
				</div>
			</div>

			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-2">
					<Label>ประเภท (Category)</Label>
					<Select.Root type="single" bind:value={formData.category} disabled={!!forcedCategory}>
						<Select.Trigger>
							{formData.category === 'administrative'
								? 'บริหารจัดการ (Administrative)'
								: formData.category === 'academic'
									? 'วิชาการ (Academic)'
									: formData.category}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="administrative">บริหารจัดการ (Administrative)</Select.Item>
							<Select.Item value="academic">วิชาการ (Academic)</Select.Item>
							<Select.Item value="miscellaneous">ทั่วไป (Miscellaneous)</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>
				<div class="space-y-2">
					<Label>ระดับหน่วยงาน (Org Type)</Label>
					<Select.Root type="single" bind:value={formData.org_type}>
						<Select.Trigger>
							{formData.org_type === 'group'
								? 'กลุ่ม (Group/Cluster)'
								: formData.org_type === 'unit'
									? 'หน่วยงาน/ฝ่าย (Unit/Dept)'
									: formData.org_type}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="group">กลุ่ม (Group/Cluster)</Select.Item>
							<Select.Item value="unit">หน่วยงาน/ฝ่าย (Unit/Dept)</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>
			</div>

			<div class="space-y-2">
				<Label>สังกัดภายใต้ (Parent Department)</Label>
				<Select.Root type="single" bind:value={formData.parent_department_id}>
					<Select.Trigger>
						{departments.find((d: Department) => d.id === formData.parent_department_id)?.name ||
							'ไม่มี (ระดับสูงสุด)'}
					</Select.Trigger>
					<Select.Content class="max-h-[200px] overflow-y-auto">
						<Select.Item value="none">ไม่มี (ระดับสูงสุด)</Select.Item>
						{#each departments.filter((d: Department) => d.id !== departmentToEdit?.id) as dept}
							<Select.Item value={dept.id}>{dept.name} ({dept.code})</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>

			<div class="space-y-2">
				<Label for="description">รายละเอียดเพิ่มเติม</Label>
				<Textarea id="description" bind:value={formData.description} />
			</div>

			<div class="grid grid-cols-3 gap-4 border-t pt-4">
				<div class="space-y-2">
					<Label for="phone">เบอร์โทร</Label>
					<Input id="phone" bind:value={formData.phone} />
				</div>
				<div class="space-y-2">
					<Label for="email">อีเมล</Label>
					<Input id="email" bind:value={formData.email} />
				</div>
				<div class="space-y-2">
					<Label for="location">สถานที่ตั้ง</Label>
					<Input id="location" bind:value={formData.location} />
				</div>
			</div>

			<Dialog.Footer>
				<Button variant="outline" type="button" onclick={() => (open = false)}>ยกเลิก</Button>
				<Button type="submit" disabled={loading}>
					{loading ? 'กำลังบันทึก...' : departmentToEdit ? 'บันทึกการแก้ไข' : 'สร้างฝ่าย'}
				</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
