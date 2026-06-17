<script lang="ts">
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import { Badge, type BadgeVariant } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { Separator } from '$lib/components/ui/separator';
	import { Textarea } from '$lib/components/ui/textarea';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import {
		createWorkItem,
		createWorkflowWindow,
		listManageableWorkflowWindows,
		updateWorkflowWindowStatus,
		type CreateWorkItemAssigneeTarget,
		type WorkflowWindow,
		type WorkflowWindowStatus
	} from '$lib/api/work';
	import {
		lookupOrganizationUnits,
		lookupStaff,
		type OrganizationUnitLookupItem,
		type StaffLookupItem
	} from '$lib/api/lookup';
	import { userPermissions, workflowManagePermissions } from '$lib/stores/permissions';
	import { toast } from 'svelte-sonner';
	import {
		BriefcaseBusiness,
		CalendarClock,
		CheckCircle2,
		LoaderCircle,
		LockKeyhole,
		Plus,
		RefreshCw,
		Send,
		TimerReset
	} from 'lucide-svelte';

	const { data }: PageProps = $props();

	type AssigneeMode = 'user' | 'organization_unit' | 'organization_position';

	const positionOptions = [
		{ value: 'director', label: 'ผู้อำนวยการ' },
		{ value: 'deputy_director', label: 'รองผู้อำนวยการ' },
		{ value: 'head', label: 'หัวหน้า' },
		{ value: 'deputy_head', label: 'รองหัวหน้า' },
		{ value: 'coordinator', label: 'ผู้ประสานงาน' },
		{ value: 'member', label: 'สมาชิก' }
	];

	let windows = $state<WorkflowWindow[]>([]);
	let staffOptions = $state<StaffLookupItem[]>([]);
	let organizationUnits = $state<OrganizationUnitLookupItem[]>([]);
	let selectedWindowId = $state('');
	let loading = $state(false);
	let savingWindow = $state(false);
	let savingItem = $state(false);

	let windowForm = $state({
		moduleCode: 'organization_work',
		workflowCode: '',
		title: '',
		description: '',
		organizationUnitId: '',
		managedByPermission: '',
		opensAt: '',
		dueAt: '',
		closesAt: ''
	});

	let itemForm = $state({
		title: '',
		description: '',
		sourceResourceType: 'manual',
		actionPath: '/staff/work',
		requiredPermission: '',
		assigneeMode: 'user' as AssigneeMode,
		userId: '',
		organizationUnitId: '',
		positionCode: 'member'
	});

	let selectedWindow = $derived(windows.find((window) => window.id === selectedWindowId) ?? null);
	let manageablePermissions = $derived(workflowManagePermissions($userPermissions));

	function statusLabel(status: WorkflowWindowStatus): string {
		switch (status) {
			case 'draft':
				return 'ฉบับร่าง';
			case 'open':
				return 'เปิดอยู่';
			case 'closed':
				return 'ปิดแล้ว';
			case 'archived':
				return 'เก็บถาวร';
		}
	}

	function statusVariant(status: WorkflowWindowStatus): BadgeVariant {
		switch (status) {
			case 'open':
				return 'default';
			case 'closed':
			case 'archived':
				return 'outline';
			default:
				return 'secondary';
		}
	}

	function assigneeModeLabel(value: AssigneeMode): string {
		switch (value) {
			case 'organization_unit':
				return 'ทั้งหน่วยงาน';
			case 'organization_position':
				return 'ตำแหน่งในหน่วยงาน';
			default:
				return 'รายบุคคล';
		}
	}

	function staffOptionLabel(id: string): string {
		return staffOptions.find((staff) => staff.id === id)?.name ?? 'เลือกบุคลากร';
	}

	function organizationUnitLabel(id: string, fallback = 'เลือกหน่วยงาน'): string {
		return organizationUnits.find((unit) => unit.id === id)?.name ?? fallback;
	}

	function positionLabel(value: string): string {
		return positionOptions.find((position) => position.value === value)?.label ?? 'สมาชิก';
	}

	function permissionLabel(value: string): string {
		return value || 'เลือก permission';
	}

	function toIsoDateTime(value: string): string | null {
		if (!value) return null;
		const date = new Date(value);
		if (Number.isNaN(date.getTime())) return null;
		return date.toISOString();
	}

	function formatDate(value?: string | null): string {
		if (!value) return '-';
		return new Date(value).toLocaleString('th-TH', {
			year: 'numeric',
			month: 'short',
			day: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		});
	}

	function selectedAssignee(): CreateWorkItemAssigneeTarget | null {
		if (itemForm.assigneeMode === 'user') {
			if (!itemForm.userId) return null;
			return { assigneeType: 'user', userId: itemForm.userId };
		}

		if (!itemForm.organizationUnitId) return null;

		if (itemForm.assigneeMode === 'organization_position') {
			return {
				assigneeType: 'organization_position',
				organizationUnitId: itemForm.organizationUnitId,
				positionCode: itemForm.positionCode
			};
		}

		return {
			assigneeType: 'organization_unit',
			organizationUnitId: itemForm.organizationUnitId
		};
	}

	async function loadData() {
		try {
			loading = true;
			const [windowItems, staff, units] = await Promise.all([
				listManageableWorkflowWindows(),
				lookupStaff({ limit: 200 }),
				lookupOrganizationUnits({ limit: 300 })
			]);

			windows = windowItems;
			staffOptions = staff;
			organizationUnits = units;
			if (!selectedWindowId && windowItems.length > 0) {
				selectedWindowId = windowItems[0].id;
			}
			if (!windowForm.managedByPermission && manageablePermissions.length > 0) {
				windowForm.managedByPermission = manageablePermissions[0];
			}
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'ไม่สามารถโหลดข้อมูลรอบงานได้');
		} finally {
			loading = false;
		}
	}

	async function submitWindow() {
		if (!windowForm.title.trim() || !windowForm.workflowCode.trim()) {
			toast.error('กรุณาระบุชื่อรอบงานและรหัสงาน');
			return;
		}
		if (!windowForm.managedByPermission) {
			toast.error('ไม่พบสิทธิ์จัดการที่สามารถใช้เปิดรอบงานได้');
			return;
		}

		try {
			savingWindow = true;
			const response = await createWorkflowWindow({
				moduleCode: windowForm.moduleCode.trim(),
				workflowCode: windowForm.workflowCode.trim(),
				title: windowForm.title.trim(),
				description: windowForm.description.trim() || null,
				organizationUnitId: windowForm.organizationUnitId || null,
				managedByPermission: windowForm.managedByPermission,
				opensAt: toIsoDateTime(windowForm.opensAt),
				dueAt: toIsoDateTime(windowForm.dueAt),
				closesAt: toIsoDateTime(windowForm.closesAt),
				metadata: { tags: [] }
			});

			if (!response.success || !response.data) {
				throw new Error(response.error || 'สร้างรอบงานไม่สำเร็จ');
			}

			toast.success('สร้างรอบงานเรียบร้อย');
			windows = [response.data, ...windows];
			selectedWindowId = response.data.id;
			windowForm.workflowCode = '';
			windowForm.title = '';
			windowForm.description = '';
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'สร้างรอบงานไม่สำเร็จ');
		} finally {
			savingWindow = false;
		}
	}

	async function setWindowStatus(window: WorkflowWindow, status: WorkflowWindowStatus) {
		try {
			const response = await updateWorkflowWindowStatus(window.id, status);
			if (!response.success || !response.data) {
				throw new Error(response.error || 'อัปเดตสถานะรอบงานไม่สำเร็จ');
			}

			windows = windows.map((item) => (item.id === window.id ? response.data! : item));
			toast.success('อัปเดตสถานะรอบงานเรียบร้อย');
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'อัปเดตสถานะรอบงานไม่สำเร็จ');
		}
	}

	async function submitWorkItem() {
		const assignee = selectedAssignee();
		if (!selectedWindow) {
			toast.error('กรุณาเลือกรอบงานก่อน');
			return;
		}
		if (!itemForm.title.trim() || !itemForm.actionPath.trim()) {
			toast.error('กรุณาระบุชื่องานและเส้นทางงาน');
			return;
		}
		if (!assignee) {
			toast.error('กรุณาเลือกผู้รับมอบหมายงาน');
			return;
		}

		try {
			savingItem = true;
			const response = await createWorkItem({
				workflowWindowId: selectedWindow.id,
				moduleCode: selectedWindow.moduleCode,
				sourceResourceType: itemForm.sourceResourceType.trim() || 'manual',
				sourceResourceId: null,
				title: itemForm.title.trim(),
				description: itemForm.description.trim() || null,
				actionPath: itemForm.actionPath.trim(),
				requiredPermission: itemForm.requiredPermission.trim() || null,
				metadata: { tags: [], sourceLabel: selectedWindow.title },
				assignees: [assignee]
			});

			if (!response.success) {
				throw new Error(response.error || 'มอบหมายงานไม่สำเร็จ');
			}

			toast.success('มอบหมายงานเรียบร้อย');
			itemForm.title = '';
			itemForm.description = '';
		} catch (error) {
			toast.error(error instanceof Error ? error.message : 'มอบหมายงานไม่สำเร็จ');
		} finally {
			savingItem = false;
		}
	}

	onMount(() => {
		void loadData();
	});
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

<section class="mx-auto flex w-full max-w-7xl flex-col gap-6 px-4 py-6 sm:px-6">
	<header class="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
		<div class="space-y-1">
			<div class="flex items-center gap-2">
				<BriefcaseBusiness class="h-7 w-7 text-primary" />
				<h1 class="text-2xl font-bold text-foreground">จัดการรอบงาน</h1>
			</div>
			<p class="text-sm text-muted-foreground">
				เปิด/ปิดรอบงานของหน่วยงาน และมอบหมายงานให้ครูหรือสมาชิกในหน่วยงาน
			</p>
		</div>

		<Button variant="outline" onclick={loadData} disabled={loading}>
			{#if loading}
				<LoaderCircle class="h-4 w-4 animate-spin" />
			{:else}
				<RefreshCw class="h-4 w-4" />
			{/if}
			รีเฟรช
		</Button>
	</header>

	<div class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_minmax(360px,420px)]">
		<div class="space-y-4">
			<section class="rounded-lg border bg-background p-4">
				<div class="flex items-center gap-2">
					<CalendarClock class="h-5 w-5 text-primary" />
					<h2 class="font-semibold">รอบงานที่จัดการได้</h2>
				</div>

					<div class="mt-4 grid gap-3">
						{#if loading}
							<PageSkeleton variant="cards" rows={3} />
						{:else if windows.length === 0}
							<PageState
								title="ยังไม่มีรอบงานที่คุณจัดการได้"
								description="สร้างรอบงานใหม่จากแบบฟอร์มด้านขวาเมื่อมี permission กลุ่มจัดการ"
							/>
						{:else}
						{#each windows as window (window.id)}
							<button
								type="button"
								class="rounded-lg border p-4 text-left transition-colors hover:bg-accent/50 {selectedWindowId ===
								window.id
									? 'border-primary bg-primary/5'
									: 'bg-background'}"
								onclick={() => {
									selectedWindowId = window.id;
								}}
							>
								<div class="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
									<div class="min-w-0 space-y-2">
										<div class="flex flex-wrap items-center gap-2">
											<Badge variant={statusVariant(window.status)}
												>{statusLabel(window.status)}</Badge
											>
											<Badge variant="outline">{window.moduleCode}</Badge>
										</div>
										<div>
											<h3 class="font-semibold text-foreground">{window.title}</h3>
											<p class="text-xs text-muted-foreground">{window.workflowCode}</p>
										</div>
										<div class="grid gap-1 text-sm text-muted-foreground sm:grid-cols-3">
											<span>เปิด {formatDate(window.opensAt)}</span>
											<span>ส่ง {formatDate(window.dueAt)}</span>
											<span>ปิด {formatDate(window.closesAt)}</span>
										</div>
									</div>

									<div class="flex gap-2">
										{#if window.status !== 'open'}
											<Button
												type="button"
												size="sm"
												variant="outline"
												onclick={(event) => {
													event.stopPropagation();
													void setWindowStatus(window, 'open');
												}}
											>
												<TimerReset class="h-4 w-4" />
												เปิด
											</Button>
										{/if}
										{#if window.status === 'open'}
											<Button
												type="button"
												size="sm"
												variant="outline"
												onclick={(event) => {
													event.stopPropagation();
													void setWindowStatus(window, 'closed');
												}}
											>
												<LockKeyhole class="h-4 w-4" />
												ปิด
											</Button>
										{/if}
									</div>
								</div>
							</button>
						{/each}
					{/if}
				</div>
			</section>

			<section class="rounded-lg border bg-background p-4">
				<div class="flex items-center gap-2">
					<Send class="h-5 w-5 text-primary" />
					<h2 class="font-semibold">มอบหมายงานในรอบที่เลือก</h2>
				</div>

				<div class="mt-4 grid gap-4">
					<div class="grid gap-2">
						<Label for="work-title">ชื่องาน</Label>
						<Input
							id="work-title"
							bind:value={itemForm.title}
							placeholder="เช่น ส่งเอกสารหลักสูตร"
						/>
					</div>

					<div class="grid gap-2">
						<Label for="work-description">รายละเอียด</Label>
						<Textarea
							id="work-description"
							bind:value={itemForm.description}
							placeholder="รายละเอียดงานหรือเงื่อนไข"
							rows={3}
						/>
					</div>

					<div class="grid gap-4 md:grid-cols-2">
						<div class="grid gap-2">
							<Label for="source-type">ประเภทแหล่งที่มา</Label>
							<Input id="source-type" bind:value={itemForm.sourceResourceType} />
						</div>
						<div class="grid gap-2">
							<Label for="action-path">ลิงก์ดำเนินการ</Label>
							<Input id="action-path" bind:value={itemForm.actionPath} />
						</div>
					</div>

					<div class="grid gap-2">
						<Label for="required-permission">สิทธิ์ที่ต้องมีเมื่อกดเข้า workflow จริง</Label>
						<Input
							id="required-permission"
							bind:value={itemForm.requiredPermission}
							placeholder="เว้นว่างได้ถ้า workflow ปลายทางตรวจเอง"
						/>
					</div>

					<Separator />

					<div class="grid gap-4 md:grid-cols-3">
						<div class="grid gap-2">
							<Label for="assignee-mode">รูปแบบผู้รับงาน</Label>
							<Select.Root type="single" bind:value={itemForm.assigneeMode}>
								<Select.Trigger id="assignee-mode" class="w-full">
									{assigneeModeLabel(itemForm.assigneeMode)}
								</Select.Trigger>
								<Select.Content>
									<Select.Item value="user">รายบุคคล</Select.Item>
									<Select.Item value="organization_unit">ทั้งหน่วยงาน</Select.Item>
									<Select.Item value="organization_position">ตำแหน่งในหน่วยงาน</Select.Item>
								</Select.Content>
							</Select.Root>
						</div>

						{#if itemForm.assigneeMode === 'user'}
							<div class="grid gap-2 md:col-span-2">
								<Label for="assignee-user">ครู/บุคลากร</Label>
								<Select.Root type="single" bind:value={itemForm.userId}>
									<Select.Trigger id="assignee-user" class="w-full">
										{staffOptionLabel(itemForm.userId)}
									</Select.Trigger>
									<Select.Content>
										<Select.Item value="">เลือกบุคลากร</Select.Item>
										{#each staffOptions as staff (staff.id)}
											<Select.Item value={staff.id}>{staff.name}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
							</div>
						{:else}
							<div class="grid gap-2">
								<Label for="assignee-unit">หน่วยงาน</Label>
								<Select.Root type="single" bind:value={itemForm.organizationUnitId}>
									<Select.Trigger id="assignee-unit" class="w-full">
										{organizationUnitLabel(itemForm.organizationUnitId)}
									</Select.Trigger>
									<Select.Content>
										<Select.Item value="">เลือกหน่วยงาน</Select.Item>
										{#each organizationUnits as unit (unit.id)}
											<Select.Item value={unit.id}>{unit.name}</Select.Item>
										{/each}
									</Select.Content>
								</Select.Root>
							</div>

							{#if itemForm.assigneeMode === 'organization_position'}
								<div class="grid gap-2">
									<Label for="assignee-position">ตำแหน่ง</Label>
									<Select.Root type="single" bind:value={itemForm.positionCode}>
										<Select.Trigger id="assignee-position" class="w-full">
											{positionLabel(itemForm.positionCode)}
										</Select.Trigger>
										<Select.Content>
											{#each positionOptions as position (position.value)}
												<Select.Item value={position.value}>{position.label}</Select.Item>
											{/each}
										</Select.Content>
									</Select.Root>
								</div>
							{/if}
						{/if}
					</div>

					<Button onclick={submitWorkItem} disabled={savingItem || !selectedWindow}>
						{#if savingItem}
							<LoaderCircle class="h-4 w-4 animate-spin" />
						{:else}
							<CheckCircle2 class="h-4 w-4" />
						{/if}
						มอบหมายงาน
					</Button>
				</div>
			</section>
		</div>

		<aside class="space-y-4">
			<section class="rounded-lg border bg-background p-4">
				<div class="flex items-center gap-2">
					<Plus class="h-5 w-5 text-primary" />
					<h2 class="font-semibold">สร้างรอบงานใหม่</h2>
				</div>

					<div class="mt-4 grid gap-4">
						{#if manageablePermissions.length === 0}
							<PageState
								variant="permission"
								title="ยังไม่มีสิทธิ์เปิดรอบงาน"
								description="บัญชีนี้ยังไม่มี permission กลุ่มจัดการ `.manage.` สำหรับเปิดรอบงาน"
							/>
						{/if}

					<div class="grid gap-2">
						<Label for="window-title">ชื่อรอบงาน</Label>
						<Input
							id="window-title"
							bind:value={windowForm.title}
							placeholder="เช่น รอบส่งเอกสารหลักสูตร ภาคเรียน 1"
						/>
					</div>

					<div class="grid gap-2">
						<Label for="workflow-code">รหัสงาน</Label>
						<Input
							id="workflow-code"
							bind:value={windowForm.workflowCode}
							placeholder="curriculum-docs-t1"
						/>
					</div>

					<div class="grid gap-2">
						<Label for="module-code">Module code</Label>
						<Input id="module-code" bind:value={windowForm.moduleCode} />
					</div>

					<div class="grid gap-2">
						<Label for="managed-permission">สิทธิ์ผู้จัดการรอบงาน</Label>
						<Select.Root
							type="single"
							bind:value={windowForm.managedByPermission}
							disabled={manageablePermissions.length === 0}
						>
							<Select.Trigger id="managed-permission" class="w-full">
								{permissionLabel(windowForm.managedByPermission)}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="">เลือก permission</Select.Item>
								{#each manageablePermissions as permission (permission)}
									<Select.Item value={permission}>{permission}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>

					<div class="grid gap-2">
						<Label for="window-unit">หน่วยงานเจ้าของรอบงาน</Label>
						<Select.Root type="single" bind:value={windowForm.organizationUnitId}>
							<Select.Trigger id="window-unit" class="w-full">
								{organizationUnitLabel(windowForm.organizationUnitId, 'ไม่ระบุ')}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="">ไม่ระบุ</Select.Item>
								{#each organizationUnits as unit (unit.id)}
									<Select.Item value={unit.id}>{unit.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>

					<div class="grid gap-2">
						<Label for="window-description">รายละเอียด</Label>
						<Textarea id="window-description" bind:value={windowForm.description} rows={3} />
					</div>

					<div class="grid gap-3">
						<div class="grid gap-2">
							<Label for="opens-at">เวลาเปิด</Label>
							<Input id="opens-at" type="datetime-local" bind:value={windowForm.opensAt} />
						</div>
						<div class="grid gap-2">
							<Label for="due-at">กำหนดส่ง</Label>
							<Input id="due-at" type="datetime-local" bind:value={windowForm.dueAt} />
						</div>
						<div class="grid gap-2">
							<Label for="closes-at">เวลาปิด</Label>
							<Input id="closes-at" type="datetime-local" bind:value={windowForm.closesAt} />
						</div>
					</div>

					<Button
						onclick={submitWindow}
						disabled={savingWindow || manageablePermissions.length === 0}
					>
						{#if savingWindow}
							<LoaderCircle class="h-4 w-4 animate-spin" />
						{:else}
							<Plus class="h-4 w-4" />
						{/if}
						สร้างรอบงาน
					</Button>
				</div>
			</section>
		</aside>
	</div>
</section>
