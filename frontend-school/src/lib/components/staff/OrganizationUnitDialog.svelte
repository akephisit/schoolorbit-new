<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Textarea } from '$lib/components/ui/textarea';
	import {
		createOrganizationUnit,
		updateOrganizationUnit,
		type OrganizationUnit,
		type CreateOrganizationUnitRequest
	} from '$lib/api/staff';
	import { toast } from 'svelte-sonner';

	let {
		open = $bindable(false),
		organizationUnitToEdit = null,
		organizationUnits = [],
		onSuccess,
		forcedCategory = undefined,
		forcedParentId = undefined
	} = $props<{
		open: boolean;
		organizationUnitToEdit?: OrganizationUnit | null;
		organizationUnits: OrganizationUnit[];
		onSuccess: () => void;
		forcedCategory?: string;
		forcedParentId?: string;
	}>();

	let loading = $state(false);

	let formData = $state({
		code: '',
		name: '',
		name_en: '',
		description: '',
		parent_unit_id: 'none',
		category: 'general',
		unit_type: 'division',
		phone: '',
		email: '',
		location: '',
		display_order: 0
	});

	// Pre-fill data when organizationUnitToEdit changes
	$effect(() => {
		if (organizationUnitToEdit) {
			formData = {
				code: organizationUnitToEdit.code,
				name: organizationUnitToEdit.name,
				name_en: organizationUnitToEdit.name_en || '',
				description: organizationUnitToEdit.description || '',
				parent_unit_id: organizationUnitToEdit.parent_unit_id || 'none',
				category: organizationUnitToEdit.category || forcedCategory || 'general',
				unit_type: organizationUnitToEdit.unit_type || 'division',
				phone: organizationUnitToEdit.phone || '',
				email: organizationUnitToEdit.email || '',
				location: organizationUnitToEdit.location || '',
				display_order: organizationUnitToEdit.display_order || 0
			};
		} else {
			formData = {
				code: '',
				name: '',
				name_en: '',
				description: '',
				parent_unit_id: forcedParentId || 'none',
				category: forcedCategory || 'general',
				unit_type: 'division',
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
			const payload: Partial<OrganizationUnit> & { parent_unit_id?: string } = { ...formData };
			if (payload.parent_unit_id === 'none') {
				delete payload.parent_unit_id;
			}

			// Convert types if needed (e.g. string -> number)
			payload.display_order = Number(payload.display_order);

			if (organizationUnitToEdit) {
				await updateOrganizationUnit(organizationUnitToEdit.id, payload);
				toast.success('อัปเดตหน่วยงานสำเร็จ');
			} else {
				await createOrganizationUnit(payload as CreateOrganizationUnitRequest);
				toast.success('สร้างหน่วยงานสำเร็จ');
			}
			open = false;
			onSuccess?.();
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'เกิดข้อผิดพลาด');
		} finally {
			loading = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-[600px] max-h-[90vh] overflow-y-auto">
		<Dialog.Header>
			<Dialog.Title>{organizationUnitToEdit ? 'แก้ไขหน่วยงาน' : 'สร้างหน่วยงานใหม่'}</Dialog.Title>
			<Dialog.Description>กรอกข้อมูลรายละเอียดของหน่วยงาน กลุ่มงาน หรือกลุ่มสาระ</Dialog.Description
			>
		</Dialog.Header>

		<form onsubmit={handleSubmit} class="space-y-4 py-4">
			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-2">
					<Label for="code">รหัสหน่วยงาน *</Label>
					<Input
						id="code"
						bind:value={formData.code}
						placeholder="เช่น ACADEMIC"
						required
						disabled={!!organizationUnitToEdit}
					/>
				</div>
				<div class="space-y-2">
					<Label for="display_order">ลำดับการแสดงผล</Label>
					<Input type="number" id="display_order" bind:value={formData.display_order} />
				</div>
			</div>

			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-2">
					<Label for="name">ชื่อหน่วยงาน (ไทย) *</Label>
					<Input
						id="name"
						bind:value={formData.name}
						placeholder="เช่น กลุ่มบริหารวิชาการ"
						required
					/>
				</div>
				<div class="space-y-2">
					<Label for="name_en">ชื่อหน่วยงาน (อังกฤษ)</Label>
					<Input id="name_en" bind:value={formData.name_en} placeholder="e.g. Academic Affairs" />
				</div>
			</div>

			<div class="grid grid-cols-2 gap-4">
				<div class="space-y-2">
					<Label>ประเภท (Category)</Label>
					<Select.Root type="single" bind:value={formData.category} disabled={!!forcedCategory}>
						<Select.Trigger>
							{formData.category === 'general'
								? 'ทั่วไป (General)'
								: formData.category === 'academic'
									? 'วิชาการ (Academic)'
									: formData.category === 'personnel'
										? 'บุคลากร (Personnel)'
										: formData.category === 'budget'
											? 'งบประมาณ (Budget)'
											: formData.category}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="general">ทั่วไป (General)</Select.Item>
							<Select.Item value="academic">วิชาการ (Academic)</Select.Item>
							<Select.Item value="personnel">บุคลากร (Personnel)</Select.Item>
							<Select.Item value="budget">งบประมาณ (Budget)</Select.Item>
							<Select.Item value="other">อื่น ๆ (Other)</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>
				<div class="space-y-2">
					<Label>ชนิดหน่วยงาน (Unit Type)</Label>
					<Select.Root type="single" bind:value={formData.unit_type}>
						<Select.Trigger>
							{formData.unit_type === 'management_group'
								? 'กลุ่มบริหาร'
								: formData.unit_type === 'division'
									? 'ฝ่าย/งาน'
									: formData.unit_type === 'subject_group'
										? 'กลุ่มสาระ'
										: formData.unit_type === 'team'
											? 'ทีม'
											: formData.unit_type}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="management_group">กลุ่มบริหาร</Select.Item>
							<Select.Item value="division">ฝ่าย/งาน</Select.Item>
							<Select.Item value="subject_group">กลุ่มสาระ</Select.Item>
							<Select.Item value="team">ทีม</Select.Item>
							<Select.Item value="custom">กำหนดเอง</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>
			</div>

			<div class="space-y-2">
				<Label>สังกัดภายใต้</Label>
				<Select.Root type="single" bind:value={formData.parent_unit_id} disabled={!!forcedParentId}>
					<Select.Trigger>
						{organizationUnits.find((d: OrganizationUnit) => d.id === formData.parent_unit_id)
							?.name || 'ไม่มี (ระดับสูงสุด)'}
					</Select.Trigger>
					<Select.Content class="max-h-[200px] overflow-y-auto">
						<Select.Item value="none">ไม่มี (ระดับสูงสุด)</Select.Item>
						{#each organizationUnits.filter((d: OrganizationUnit) => d.id !== organizationUnitToEdit?.id) as dept (dept.id)}
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
					{loading ? 'กำลังบันทึก...' : organizationUnitToEdit ? 'บันทึกการแก้ไข' : 'สร้างหน่วยงาน'}
				</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
