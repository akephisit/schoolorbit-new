<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import {
		createOrganizationUnit,
		updateOrganizationUnit,
		type OrganizationUnit,
		type CreateOrganizationUnitRequest,
		type UpdateOrganizationUnitRequest
	} from '#lib/api/staff.js';
	import { onDestroy, untrack } from 'svelte';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
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
	let formError = $state('');
	let disposed = false,
		ownerEpoch = 0;
	const canSave = $derived(
		organizationUnitToEdit
			? $can.has(PERMISSIONS.ROLES_UPDATE_ALL)
			: $can.has(PERMISSIONS.ROLES_CREATE_ALL)
	);

	let formData = $state({
		name: '',
		name_en: '',
		description: '',
		parent_unit_id: 'none',
		category: 'general',
		unit_type: 'division',
		phone: '',
		email: '',
		location: ''
	});

	// Pre-fill data when organizationUnitToEdit changes
	$effect.pre(() => {
		const visible = open,
			unitId = organizationUnitToEdit?.id,
			parent = forcedParentId;
		untrack(() => {
			ownerEpoch++;
			loading = false;
			formError = '';
			if (unitId && organizationUnitToEdit) {
				formData = {
					name: organizationUnitToEdit.name,
					name_en: organizationUnitToEdit.name_en || '',
					description: organizationUnitToEdit.description || '',
					parent_unit_id: organizationUnitToEdit.parent_unit_id || 'none',
					category: organizationUnitToEdit.category || forcedCategory || 'general',
					unit_type: organizationUnitToEdit.unit_type || 'division',
					phone: organizationUnitToEdit.phone || '',
					email: organizationUnitToEdit.email || '',
					location: organizationUnitToEdit.location || ''
				};
			} else {
				formData = {
					name: '',
					name_en: '',
					description: '',
					parent_unit_id: forcedParentId || 'none',
					category: forcedCategory || 'general',
					unit_type: 'division',
					phone: '',
					email: '',
					location: ''
				};
			}
			void visible;
			void parent;
			void unitId;
		});
	});
	onDestroy(() => {
		disposed = true;
		ownerEpoch++;
	});

	async function handleSubmit(e: SubmitEvent) {
		e.preventDefault();
		if (!open || !canSave || loading) return;
		const target = organizationUnitToEdit?.id,
			epoch = ownerEpoch;
		const current = () => !disposed && epoch === ownerEpoch;
		loading = true;

		try {
			formError = '';
			const payload: CreateOrganizationUnitRequest & UpdateOrganizationUnitRequest = {
				name: formData.name.trim(),
				name_en: formData.name_en.trim(),
				description: formData.description,
				category: formData.category,
				unit_type: formData.unit_type,
				phone: formData.phone,
				email: formData.email,
				location: formData.location,
				parent_unit_id: formData.parent_unit_id === 'none' ? undefined : formData.parent_unit_id
			};
			const response = target
				? await updateOrganizationUnit(target, payload)
				: await createOrganizationUnit(payload);
			if (!current()) return;
			if (!response.success) throw new Error(response.error ?? 'บันทึกหน่วยงานไม่สำเร็จ');
			toast.success(target ? 'อัปเดตหน่วยงานสำเร็จ' : 'สร้างหน่วยงานสำเร็จ');
			open = false;
			onSuccess?.();
		} catch (error) {
			if (!current()) return;
			formError = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
		} finally {
			if (current()) loading = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-[600px] max-h-[90vh] overflow-y-auto">
		<Dialog.Header>
			<Dialog.Title>{organizationUnitToEdit ? 'แก้ไขหน่วยงาน' : 'สร้างหน่วยงานใหม่'}</Dialog.Title>
			<Dialog.Description>ระบุชื่อและสังกัดของกลุ่มบริหารหรือฝ่าย/งาน</Dialog.Description>
		</Dialog.Header>

		<form onsubmit={handleSubmit} class="space-y-4 py-4">
			<fieldset disabled={loading} class="contents">
				<div class="grid gap-4 sm:grid-cols-2">
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

				<div class="grid gap-4 sm:grid-cols-2">
					<div class="space-y-2">
						<Label for="unit-category">หมวดงาน</Label>
						<Select.Root type="single" bind:value={formData.category} disabled={!!forcedCategory}>
							<Select.Trigger id="unit-category">
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
						<Label for="unit-kind">ประเภทหน่วยงาน</Label>
						<Select.Root
							type="single"
							bind:value={formData.unit_type}
							disabled={organizationUnitToEdit?.is_system}
						>
							<Select.Trigger id="unit-kind">
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
								<Select.Item value="team">ทีม</Select.Item>
								<Select.Item value="custom">กำหนดเอง</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>
				</div>

				<div class="space-y-2">
					<Label for="unit-parent">สังกัดภายใต้</Label>
					<Select.Root
						type="single"
						bind:value={formData.parent_unit_id}
						disabled={!!forcedParentId}
					>
						<Select.Trigger id="unit-parent">
							{organizationUnits.find((d: OrganizationUnit) => d.id === formData.parent_unit_id)
								?.name || 'ระดับโรงเรียน'}
						</Select.Trigger>
						<Select.Content class="max-h-[200px] overflow-y-auto">
							<Select.Item value="none">ระดับโรงเรียน</Select.Item>
							{#each organizationUnits.filter((d: OrganizationUnit) => d.id !== organizationUnitToEdit?.id && d.is_active && d.unit_type !== 'subject_group') as dept (dept.id)}
								<Select.Item value={dept.id}>{dept.name} ({dept.code})</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				<div class="space-y-2">
					<Label for="description">รายละเอียดเพิ่มเติม</Label>
					<Textarea id="description" bind:value={formData.description} />
				</div>

				<div class="grid gap-4 sm:grid-cols-3 border-t pt-4">
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

				{#if formError}<p role="alert" class="text-sm text-destructive">{formError}</p>{/if}
			</fieldset>
			<Dialog.Footer>
				<Button variant="outline" type="button" onclick={() => (open = false)}>ยกเลิก</Button>
				<LoadingButton type="submit" {loading}>
					{organizationUnitToEdit ? 'บันทึกการแก้ไข' : 'สร้างหน่วยงาน'}
				</LoadingButton>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
