<script lang="ts">
	import { ChevronDown } from '@lucide/svelte';
	import * as Collapsible from '#lib/components/ui/collapsible/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
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
	import {
		organizationAncestors,
		organizationUnitLabel
	} from '#lib/features/staff/organization-hierarchy.js';

	let {
		open = $bindable(false),
		organizationUnitToEdit = null,
		organizationUnits = [],
		onSuccess,
		forcedCategory = undefined,
		forcedParentId = undefined
	}: {
		open: boolean;
		organizationUnitToEdit?: OrganizationUnit | null;
		organizationUnits: OrganizationUnit[];
		onSuccess: () => void;
		forcedCategory?: string;
		forcedParentId?: string;
	} = $props();

	let loading = $state(false);
	let formError = $state('');
	let extrasOpen = $state(false);
	const categories = [
		{ value: 'general', label: 'ทั่วไป' },
		{ value: 'academic', label: 'วิชาการ' },
		{ value: 'personnel', label: 'บุคลากร' },
		{ value: 'budget', label: 'งบประมาณ' },
		{ value: 'administrative', label: 'บริหาร' },
		{ value: 'student_affairs', label: 'กิจการนักเรียน' },
		{ value: 'other', label: 'อื่น ๆ' }
	];
	const unitKinds = [
		{ value: 'management_group', label: 'กลุ่มบริหาร' },
		{ value: 'division', label: 'ฝ่าย/งาน' },
		{ value: 'team', label: 'ทีม' },
		{ value: 'committee', label: 'คณะกรรมการ' },
		{ value: 'custom', label: 'หน่วยงานอื่น' }
	];
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
			extrasOpen = !!(
				organizationUnitToEdit?.name_en ||
				organizationUnitToEdit?.description ||
				organizationUnitToEdit?.phone ||
				organizationUnitToEdit?.email ||
				organizationUnitToEdit?.location
			);
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
				parent_unit_id: organizationUnitToEdit?.is_system
					? undefined
					: formData.parent_unit_id === 'none'
						? organizationUnits.find((unit: OrganizationUnit) => unit.unit_type === 'school')?.id
						: formData.parent_unit_id
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
	<Dialog.Content class="flex max-h-[90dvh] flex-col gap-0 overflow-hidden p-0 sm:max-w-2xl">
		<Dialog.Header
			class="shrink-0 border-b bg-gradient-to-br from-primary/5 via-card to-card px-6 py-5 pr-10"
		>
			<Dialog.Title>{organizationUnitToEdit ? 'แก้ไขหน่วยงาน' : 'สร้างหน่วยงานใหม่'}</Dialog.Title>
			<Dialog.Description
				>ระบุชื่อ ประเภท และสังกัดของหน่วยงาน ข้อมูลเพิ่มเติมกรอกภายหลังได้</Dialog.Description
			>
		</Dialog.Header>
		<form onsubmit={handleSubmit} class="flex min-h-0 flex-1 flex-col">
			<div class="min-h-0 flex-1 overflow-y-auto px-6 py-5">
				<fieldset disabled={loading} class="min-w-0 space-y-5">
					<section class="space-y-4" aria-label="ข้อมูลหลักของหน่วยงาน">
						<div class="space-y-2">
							<Label for="name">ชื่อหน่วยงาน (ไทย) *</Label>
							<Input
								id="name"
								bind:value={formData.name}
								placeholder="เช่น กลุ่มบริหารวิชาการ หรืองานทะเบียน"
								required
							/>
						</div>
						<div class="space-y-2">
							<Label>ประเภทหน่วยงาน</Label>
							{#if organizationUnitToEdit?.is_system}<p class="text-sm">
									{organizationUnitLabel(organizationUnitToEdit)}
								</p>
							{:else}<ToggleGroup.Root
									type="single"
									aria-label="ประเภทหน่วยงาน"
									class="flex-wrap justify-start"
									bind:value={
										() => formData.unit_type,
										(value) => {
											if (unitKinds.some((kind) => kind.value === value))
												formData.unit_type = value;
										}
									}
								>
									{#each unitKinds as kind (kind.value)}<ToggleGroup.Item
											value={kind.value}
											disabled={loading}>{kind.label}</ToggleGroup.Item
										>{/each}
								</ToggleGroup.Root>{/if}
						</div>
						<div class="space-y-2">
							<Label for="unit-parent">สังกัดภายใต้</Label>
							<Select.Root
								type="single"
								bind:value={formData.parent_unit_id}
								disabled={!!forcedParentId || !!organizationUnitToEdit?.is_system || loading}
							>
								<Select.Trigger
									id="unit-parent"
									class="w-full min-w-0 whitespace-normal text-left data-[size=default]:h-auto"
								>
									<span class="min-w-0 break-words"
										>{organizationUnits.find(
											(unit: OrganizationUnit) => unit.id === formData.parent_unit_id
										)?.name || 'ระดับโรงเรียน'}</span
									>
								</Select.Trigger>
								<Select.Content class="max-h-64 w-(--bits-select-anchor-width) overflow-y-auto">
									<Select.Item value="none">ระดับโรงเรียน</Select.Item>
									{#each organizationUnits.filter((unit: OrganizationUnit) => unit.id !== organizationUnitToEdit?.id && unit.is_active && unit.unit_type !== 'subject_group') as unit (unit.id)}
										<Select.Item value={unit.id}
											>{[...organizationAncestors(unit, organizationUnits), unit]
												.map((item) => item.name)
												.join(' / ')}</Select.Item
										>
									{/each}
								</Select.Content>
							</Select.Root>
							<p class="text-xs text-muted-foreground">
								เลือกกลุ่มบริหารหรือหน่วยงานที่ดูแลส่วนนี้โดยตรง
							</p>
						</div>
						<div class="space-y-2">
							<Label for="unit-category">หมวดงาน</Label>
							<Select.Root
								type="single"
								bind:value={formData.category}
								disabled={!!forcedCategory || loading}
							>
								<Select.Trigger id="unit-category" class="w-full"
									>{categories.find((category) => category.value === formData.category)?.label ??
										'อื่น ๆ'}</Select.Trigger
								>
								<Select.Content
									>{#each categories as category (category.value)}<Select.Item
											value={category.value}>{category.label}</Select.Item
										>{/each}</Select.Content
								>
							</Select.Root>
						</div>
					</section>
					<Collapsible.Root bind:open={extrasOpen} class="rounded-xl border">
						<Collapsible.Trigger
							>{#snippet child({ props })}<Button
									{...props}
									type="button"
									variant="ghost"
									class="h-auto w-full justify-between whitespace-normal px-4 py-3 text-left"
									disabled={loading}>ข้อมูลเพิ่มเติม <ChevronDown class="size-4" /></Button
								>{/snippet}</Collapsible.Trigger
						>
						<Collapsible.Content class="space-y-4 border-t p-4">
							<div class="space-y-2">
								<Label for="name_en">ชื่อหน่วยงาน (อังกฤษ)</Label><Input
									id="name_en"
									bind:value={formData.name_en}
									placeholder="เช่น Academic Affairs"
								/>
							</div>
							<div class="space-y-2">
								<Label for="description">รายละเอียดเพิ่มเติม</Label><Textarea
									id="description"
									bind:value={formData.description}
								/>
							</div>
							<div class="grid gap-4 sm:grid-cols-2">
								<div class="space-y-2">
									<Label for="phone">เบอร์โทร</Label><Input
										id="phone"
										type="tel"
										bind:value={formData.phone}
									/>
								</div>
								<div class="space-y-2">
									<Label for="email">อีเมล</Label><Input id="email" bind:value={formData.email} />
								</div>
							</div>
							<div class="space-y-2">
								<Label for="location">สถานที่ตั้ง</Label><Input
									id="location"
									bind:value={formData.location}
								/>
							</div>
						</Collapsible.Content>
					</Collapsible.Root>
				</fieldset>
			</div>
			{#if formError}<p role="alert" class="px-6 py-3 text-sm text-destructive">{formError}</p>{/if}
			<Dialog.Footer class="shrink-0 border-t bg-background px-6 py-4">
				<Button variant="outline" type="button" onclick={() => (open = false)}>ยกเลิก</Button>
				<LoadingButton type="submit" {loading} disabled={!canSave}
					>{organizationUnitToEdit ? 'บันทึกการแก้ไข' : 'สร้างหน่วยงาน'}</LoadingButton
				>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
