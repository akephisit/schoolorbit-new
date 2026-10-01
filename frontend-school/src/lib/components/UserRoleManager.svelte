<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad, type RouteLoadResult } from '$lib/navigation/route-load';
	import { requireApiData } from '$lib/api/client';
	import {
		PERMISSIONS,
		PERMISSION_MODULES,
		permissionActionLabel,
		permissionScopeMeta
	} from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { userRoleAPI, roleAPI, type UserRoleAssignment, type Role } from '$lib/api/roles';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { Label } from '$lib/components/ui/label';
	import { Input } from '$lib/components/ui/input';
	import {
		Dialog,
		DialogContent,
		DialogDescription,
		DialogFooter,
		DialogHeader,
		DialogTitle
	} from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { Shield, Plus, Trash2, Star } from '@lucide/svelte';
	import { toast } from 'svelte-sonner';

	interface Props {
		userId: string;
		rolesRead: Promise<RouteLoadResult<UserRoleAssignment[]>>;
		permissionsRead: Promise<RouteLoadResult<string[]>>;
	}

	let { userId, rolesRead, permissionsRead }: Props = $props();

	let userRoles = $state<UserRoleAssignment[]>([]);
	let availableRoles = $state<Role[]>([]);
	let permissions = $state<string[]>([]);
	let permissionSearch = $state('');
	const permissionModules: Record<string, string> = {
		[PERMISSION_MODULES.STAFF_PROFILE]: 'ข้อมูลบุคลากร',
		[PERMISSION_MODULES.STAFF_PII]: 'ข้อมูลส่วนบุคคลของบุคลากร',
		[PERMISSION_MODULES.STAFF]: 'จัดการบุคลากร',
		[PERMISSION_MODULES.STUDENT]: 'ข้อมูลนักเรียน',
		[PERMISSION_MODULES.STUDENT_PII]: 'ข้อมูลส่วนบุคคลของนักเรียน',
		[PERMISSION_MODULES.ROLES]: 'บทบาทและสิทธิ์',
		[PERMISSION_MODULES.CERTIFICATE]: 'เกียรติบัตร',
		[PERMISSION_MODULES.ACHIEVEMENT]: 'ผลงานและรางวัล',
		[PERMISSION_MODULES.CALENDAR]: 'ปฏิทิน',
		[PERMISSION_MODULES.ADMISSION]: 'รับสมัครนักเรียน',
		[PERMISSION_MODULES.ORGANIZATION_WORK]: 'งานของหน่วยงาน',
		[PERMISSION_MODULES.SUPERVISION]: 'การนิเทศ',
		[PERMISSION_MODULES.SETTINGS]: 'ตั้งค่าระบบ',
		[PERMISSION_MODULES.MENU]: 'เมนูบริการ',
		[PERMISSION_MODULES.HOMEROOM]: 'ห้องประจำชั้น',
		[PERMISSION_MODULES.LEARNING_OFFERING]: 'การจัดการเรียนรู้',
		[PERMISSION_MODULES.ACADEMIC_CATALOG]: 'รายวิชา',
		[PERMISSION_MODULES.ACADEMIC_CURRICULUM]: 'หลักสูตร',
		[PERMISSION_MODULES.ACADEMIC_ASSESSMENT]: 'การวัดผล',
		[PERMISSION_MODULES.ACADEMIC_GRADEBOOK]: 'บันทึกคะแนน',
		[PERMISSION_MODULES.ACADEMIC_TIMETABLE]: 'ตารางสอน',
		[PERMISSION_MODULES.ACADEMIC_EXAM_SCHEDULE]: 'ตารางสอบ',
		[PERMISSION_MODULES.ACADEMIC_RESULT]: 'ผลการเรียน',
		[PERMISSION_MODULES.ACADEMIC_QUESTION_BANK]: 'คลังข้อสอบ',
		[PERMISSION_MODULES.ACADEMIC_YEAR]: 'ปีการศึกษา',
		[PERMISSION_MODULES.ACADEMIC_TERM]: 'ภาคเรียน',
		[PERMISSION_MODULES.ACADEMIC_PROMOTION]: 'การเลื่อนชั้น',
		[PERMISSION_MODULES.ACADEMIC_LIFECYCLE]: 'ปิดปีและภาคเรียน',
		[PERMISSION_MODULES.ACADEMIC_LEARNER_EVALUATION]: 'ประเมินผู้เรียน',
		[PERMISSION_MODULES.ACADEMIC_CONTEXT]: 'บริบทการศึกษา',
		[PERMISSION_MODULES.ACADEMIC_TIMETABLE_TODAY]: 'ตารางสอนประจำวัน',
		[PERMISSION_MODULES.STUDENT_ACADEMIC_YEAR]: 'นักเรียนในปีการศึกษา',
		[PERMISSION_MODULES.FONT]: 'แบบอักษร',
		[PERMISSION_MODULES.FACILITY]: 'สถานที่',
		[PERMISSION_MODULES.FEATURES]: 'บริการของโรงเรียน',
		[PERMISSION_MODULES.DASHBOARD]: 'ภาพรวม',
		[PERMISSION_MODULES.SYSTEM]: 'ระบบ'
	};
	const permissionGroups = $derived.by(() => {
		const groups = new SvelteMap<string, string[]>();
		const search = permissionSearch.trim().toLowerCase();
		for (const code of permissions) {
			const [module, action, scope] = code.split('.');
			const label = permissionModules[module] ?? module;
			if (
				search &&
				!`${code} ${label} ${permissionActionLabel(action)} ${permissionScopeMeta(scope).label}`
					.toLowerCase()
					.includes(search)
			)
				continue;
			const group = groups.get(label) ?? [];
			group.push(code);
			groups.set(label, group);
		}
		return Array.from(groups.entries());
	});
	let loading = $state(true);
	let rolesLoaded = $state(false),
		permissionsLoaded = $state(false),
		effectivePermissionsLoading = $state(true);
	let rolesError = $state(''),
		permissionsError = $state(''),
		catalogError = $state(''),
		catalogLoading = $state(false);
	const rolesRequest = new LatestRequest(),
		permissionsRequest = new LatestRequest(),
		catalogRequest = new LatestRequest();
	const canRead = $derived($can.has(PERMISSIONS.ROLES_READ_ALL));
	const canAssign = $derived($can.has(PERMISSIONS.ROLES_ASSIGN_ALL));
	const canRemove = $derived($can.has(PERMISSIONS.ROLES_REMOVE_ALL));
	let activeId = '',
		disposed = false,
		ownerEpoch = 0,
		dialogEpoch = 0;
	let showAssignDialog = $state(false);
	let selectedRoleId = $state<string | undefined>(undefined);
	let isPrimary = $state(false);
	let assigning = $state(false);

	$effect.pre(() => {
		const id = userId;
		untrack(() => {
			if (activeId !== id) {
				activeId = id;
				ownerEpoch++;
				userRoles = [];
				permissions = [];
				permissionSearch = '';
				rolesLoaded = false;
				permissionsLoaded = false;
				showAssignDialog = false;
				assigning = false;
			}
		});
	});
	$effect.pre(() => {
		const read = rolesRead;
		untrack(() => {
			const ticket = rolesRequest.begin();
			loading = true;
			rolesError = '';
			void read.then((result) => applyRoles(result, ticket.revision));
		});
		return () => rolesRequest.abort();
	});
	$effect.pre(() => {
		const read = permissionsRead;
		untrack(() => {
			const ticket = permissionsRequest.begin();
			effectivePermissionsLoading = true;
			permissionsError = '';
			void read.then((result) => applyPermissions(result, ticket.revision));
		});
		return () => permissionsRequest.abort();
	});
	$effect.pre(() => {
		const open = showAssignDialog,
			id = userId,
			allowed = canRead && canAssign;
		untrack(() => {
			dialogEpoch++;
			selectedRoleId = undefined;
			isPrimary = false;
			availableRoles = [];
			catalogError = '';
			if (open && allowed && id) void loadAvailableRoles();
			else {
				catalogRequest.abort();
				catalogLoading = false;
			}
		});
		return () => catalogRequest.abort();
	});
	onDestroy(() => {
		disposed = true;
		ownerEpoch++;
		rolesRequest.abort();
		permissionsRequest.abort();
		catalogRequest.abort();
	});
	function applyRoles(result: RouteLoadResult<UserRoleAssignment[]>, revision: number) {
		if (!rolesRequest.isCurrent(revision)) return;
		loading = false;
		if (result.ok) {
			userRoles = result.data;
			rolesLoaded = true;
		} else rolesError = result.error;
	}
	function applyPermissions(result: RouteLoadResult<string[]>, revision: number) {
		if (!permissionsRequest.isCurrent(revision)) return;
		effectivePermissionsLoading = false;
		if (result.ok) {
			permissions = result.data;
			permissionsLoaded = true;
		} else permissionsError = result.error;
	}
	async function loadUserRoles() {
		if (!canRead) return;
		const ticket = rolesRequest.begin();
		loading = true;
		rolesError = '';
		applyRoles(
			await captureRouteLoad(
				userRoleAPI
					.getUserRoles(userId, { signal: ticket.signal })
					.then((reply) => requireApiData(reply, 'โหลดบทบาทที่ได้รับไม่สำเร็จ')),
				'โหลดบทบาทที่ได้รับไม่สำเร็จ'
			),
			ticket.revision
		);
	}
	async function loadPermissions() {
		if (!canRead) return;
		const ticket = permissionsRequest.begin();
		effectivePermissionsLoading = true;
		permissionsError = '';
		applyPermissions(
			await captureRouteLoad(
				userRoleAPI
					.getUserPermissions(userId, { signal: ticket.signal })
					.then((reply) => requireApiData(reply, 'โหลดสิทธิ์ที่มีผลไม่สำเร็จ')),
				'โหลดสิทธิ์ที่มีผลไม่สำเร็จ'
			),
			ticket.revision
		);
	}
	async function loadAvailableRoles() {
		if (!showAssignDialog || !canRead || !canAssign) return;
		const ticket = catalogRequest.begin();
		catalogLoading = true;
		catalogError = '';
		const result = await captureRouteLoad(
			roleAPI
				.listRoles({}, { signal: ticket.signal })
				.then((reply) => requireApiData(reply, 'โหลดตัวเลือกบทบาทไม่สำเร็จ')),
			'โหลดตัวเลือกบทบาทไม่สำเร็จ'
		);
		if (!catalogRequest.isCurrent(ticket.revision)) return;
		catalogLoading = false;
		if (result.ok) availableRoles = result.data.filter((role) => role.is_active);
		else catalogError = result.error;
	}
	async function handleAssignRole() {
		if (!canAssign || !selectedRoleId || assigning || !showAssignDialog) return;
		const target = userId,
			epoch = ownerEpoch,
			draft = dialogEpoch;
		const current = () => !disposed && epoch === ownerEpoch && target === userId;
		rolesRequest.abort();
		permissionsRequest.abort();
		loading = false;
		effectivePermissionsLoading = false;
		assigning = true;
		try {
			const reply = await userRoleAPI.assignRole(target, {
				role_id: selectedRoleId,
				is_primary: isPrimary,
				started_at: new Date().toISOString().split('T')[0]
			});
			if (!current()) return;
			if (!reply.success) throw new Error(reply.error || 'ไม่สามารถมอบหมายบทบาทได้');
			if (draft === dialogEpoch) {
				toast.success('มอบหมายบทบาทสำเร็จ');
				showAssignDialog = false;
			}
			await Promise.all([loadUserRoles(), loadPermissions()]);
		} catch (e) {
			if (current() && draft === dialogEpoch)
				toast.error(e instanceof Error ? e.message : 'ไม่สามารถมอบหมายบทบาทได้');
		} finally {
			if (current()) assigning = false;
		}
	}
	async function handleRemoveRole(roleId: string, roleName: string) {
		if (!canRemove || assigning || !confirm(`คุณแน่ใจหรือไม่ที่จะเพิกถอนบทบาท "${roleName}"?`))
			return;
		const target = userId,
			epoch = ownerEpoch;
		const current = () => !disposed && epoch === ownerEpoch && target === userId;
		rolesRequest.abort();
		permissionsRequest.abort();
		loading = false;
		effectivePermissionsLoading = false;
		assigning = true;
		try {
			const reply = await userRoleAPI.removeRole(target, roleId);
			if (!current()) return;
			if (!reply.success) throw new Error(reply.error || 'ไม่สามารถเพิกถอนบทบาทได้');
			toast.success('เพิกถอนบทบาทสำเร็จ');
			await Promise.all([loadUserRoles(), loadPermissions()]);
		} catch (e) {
			if (current()) toast.error(e instanceof Error ? e.message : 'ไม่สามารถเพิกถอนบทบาทได้');
		} finally {
			if (current()) assigning = false;
		}
	}

	function getUnassignedRoles(): Role[] {
		const assignedRoleIds = new Set(userRoles.map((ur) => ur.role_id));
		return availableRoles.filter((r) => r.user_type === 'staff' && !assignedRoleIds.has(r.id));
	}
</script>

<div class="space-y-6">
	<section data-testid="user-role-assignments" aria-busy={loading}>
		<Card>
			<CardHeader>
				<div class="flex items-center justify-between">
					<div>
						<CardTitle>บทบาทที่ได้รับ</CardTitle>
						<CardDescription>บทบาทและสิทธิ์การเข้าถึงของผู้ใช้งาน</CardDescription>
					</div>
					{#if canAssign}<Button
							onclick={() => (showAssignDialog = true)}
							disabled={assigning}
							size="sm"
							class="gap-2"
						>
							<Plus class="h-4 w-4" />
							เพิ่มบทบาท
						</Button>{/if}
				</div>
			</CardHeader>
			<CardContent>
				{#if rolesError && rolesLoaded && canRead}<PageState
						title="อัปเดตบทบาทไม่สำเร็จ"
						description={rolesError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadUserRoles}
					/>{/if}
				{#if loading && rolesLoaded && canRead}<p role="status">กำลังอัปเดตบทบาท...</p>{/if}
				{#if !canRead}<PageState variant="permission" title="ไม่มีสิทธิ์อ่านบทบาทที่ได้รับ" />
				{:else if loading && !rolesLoaded}<div role="status" aria-label="กำลังโหลดบทบาทที่ได้รับ">
						<PageSkeleton variant="cards" rows={2} />
					</div>
				{:else if rolesError && !rolesLoaded}<PageState
						variant="error"
						title="โหลดบทบาทที่ได้รับไม่สำเร็จ"
						description={rolesError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadUserRoles}
					/>
				{:else if userRoles.length === 0}
					<div class="text-center py-8">
						<Shield class="h-12 w-12 text-muted-foreground mx-auto mb-2" />
						<p class="text-muted-foreground">ยังไม่มีบทบาทที่ได้รับ</p>
						{#if canAssign}<Button
								onclick={() => (showAssignDialog = true)}
								variant="outline"
								size="sm"
								class="mt-4 gap-2"
							>
								<Plus class="h-4 w-4" />
								เพิ่มบทบาท
							</Button>{/if}
					</div>
				{:else}
					<div class="space-y-2">
						{#each userRoles as userRole (userRole.id)}
							{@const role = userRole.role}
							<div
								class="flex flex-wrap items-center justify-between gap-3 p-3 border rounded-lg hover:bg-muted/50"
							>
								<div class="flex items-center gap-3 flex-1">
									{#if userRole.is_primary}
										<Star class="h-4 w-4 text-yellow-500 fill-yellow-500" />
									{:else}
										<Shield class="h-4 w-4 text-muted-foreground" />
									{/if}
									<div class="flex-1 min-w-0">
										<div class="flex items-center gap-2">
											<p class="font-medium text-foreground">{role.name}</p>
											{#if userRole.is_primary}
												<Badge variant="secondary" class="text-xs">หลัก</Badge>
											{/if}
										</div>
										<p class="text-sm text-muted-foreground">{role.code}</p>
									</div>
									<div class="text-sm text-muted-foreground">
										{role.permissions.includes('*')
											? 'ทุกสิทธิ์'
											: `${role.permissions.length} สิทธิ์`}
									</div>
								</div>
								{#if canRemove}<Button
										variant="ghost"
										size="sm"
										onclick={() => handleRemoveRole(role.id, role.name)}
										disabled={assigning}
										class="gap-1 text-red-600 hover:text-red-700 hover:bg-red-50"
									>
										<Trash2 class="h-3 w-3" />
										เพิกถอน
									</Button>{/if}
							</div>
						{/each}
					</div>
				{/if}
			</CardContent>
		</Card>
	</section>
	<section data-testid="user-effective-permissions" aria-busy={effectivePermissionsLoading}>
		<Card>
			<CardHeader>
				<CardTitle>สิทธิ์ที่มีผล</CardTitle>
				<CardDescription
					>สิทธิ์จากบทบาทที่ใช้งาน • สิทธิ์จากสังกัดและการมอบหมายงานตรวจสอบในหน้าที่เกี่ยวข้อง</CardDescription
				>
			</CardHeader>
			<CardContent>
				{#if permissionsError && permissionsLoaded && canRead}<PageState
						title="อัปเดตสิทธิ์ไม่สำเร็จ"
						description={permissionsError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadPermissions}
					/>{/if}
				{#if effectivePermissionsLoading && permissionsLoaded && canRead}<p role="status">
						กำลังอัปเดตสิทธิ์...
					</p>{/if}
				{#if !canRead}<PageState variant="permission" title="ไม่มีสิทธิ์อ่านสิทธิ์ที่มีผล" />
				{:else if effectivePermissionsLoading && !permissionsLoaded}<div
						role="status"
						aria-label="กำลังโหลดสิทธิ์ที่มีผล"
					>
						<PageSkeleton variant="cards" rows={2} />
					</div>
				{:else if permissionsError && !permissionsLoaded}<PageState
						variant="error"
						title="โหลดสิทธิ์ที่มีผลไม่สำเร็จ"
						description={permissionsError}
						actionLabel="ลองอีกครั้ง"
						onaction={loadPermissions}
					/>
				{:else if permissions.length === 0}
					<p class="text-center text-muted-foreground py-4">ยังไม่มีสิทธิ์</p>
				{:else if permissions.includes('*')}
					<div class="text-center py-4">
						<Badge class="bg-primary text-primary-foreground">ทุกสิทธิ์</Badge>
						<p class="text-sm text-muted-foreground mt-2">มีสิทธิ์เข้าถึงทุกอย่าง</p>
					</div>
				{:else}
					<div class="space-y-4">
						<Input
							bind:value={permissionSearch}
							aria-label="ค้นหาสิทธิ์จากบทบาท"
							placeholder="ค้นหางาน การทำงาน หรือขอบเขตสิทธิ์..."
						/>
						{#each permissionGroups as [module, codes] (module)}
							<div class="rounded-xl border p-4">
								<h3 class="mb-3 font-medium">
									{module} <span class="text-sm text-muted-foreground">({codes.length})</span>
								</h3>
								<ul class="grid gap-3 sm:grid-cols-2">
									{#each codes as code (code)}{@const parts = code.split('.')}{@const scope =
											permissionScopeMeta(parts[2])}
										<li>
											<div class="flex flex-wrap items-center gap-2">
												<span class="text-sm">{permissionActionLabel(parts[1])}</span><Badge
													variant="outline"
													title={scope.description}>{scope.label}</Badge
												>
											</div>
											<p class="mt-1 break-all text-xs text-muted-foreground">{code}</p>
										</li>{/each}
								</ul>
							</div>
						{:else}<p class="text-sm text-muted-foreground">ไม่พบสิทธิ์ที่ตรงกับคำค้น</p>{/each}
					</div>
				{/if}
			</CardContent>
		</Card>
	</section>
</div>

{#if canAssign}<Dialog bind:open={showAssignDialog}>
		<DialogContent>
			<DialogHeader>
				<DialogTitle>เพิ่มบทบาท</DialogTitle>
				<DialogDescription>เลือกบทบาทที่ต้องการมอบหมายให้ผู้ใช้งาน</DialogDescription>
			</DialogHeader>

			{#if !canRead}<PageState variant="permission" title="ไม่มีสิทธิ์อ่านตัวเลือกบทบาท" />
			{:else if catalogLoading}<div role="status" aria-label="กำลังโหลดตัวเลือกบทบาท">
					<PageSkeleton variant="form" rows={2} />
				</div>
			{:else if catalogError}<PageState
					variant="error"
					title="โหลดตัวเลือกบทบาทไม่สำเร็จ"
					description={catalogError}
					actionLabel="ลองอีกครั้ง"
					onaction={loadAvailableRoles}
				/>
			{:else}
				<div class="space-y-4 py-4">
					<div class="space-y-2">
						<Label for="role">เลือกบทบาท</Label>
						<Select.Root type="single" bind:value={selectedRoleId}>
							<Select.Trigger id="role" class="w-full">
								{@const selectedRole = availableRoles.find((role) => role.id === selectedRoleId)}
								{selectedRole ? `${selectedRole.name} (${selectedRole.code})` : 'เลือกบทบาท...'}
							</Select.Trigger>
							<Select.Content>
								{#each getUnassignedRoles() as role (role.id)}
									<Select.Item value={role.id}>{role.name} ({role.code})</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</div>

					<div class="flex items-center gap-2">
						<input type="checkbox" id="is_primary" bind:checked={isPrimary} class="rounded" />
						<Label for="is_primary">ตั้งเป็นบทบาทหลัก</Label>
					</div>
					{#if selectedRoleId}{@const selected = availableRoles.find(
							(role) => role.id === selectedRoleId
						)}
						<div class="rounded-xl border bg-muted/30 p-3 text-sm">
							<p class="font-medium">จะเพิ่มบทบาท: {selected?.name}</p>
							<p class="mt-1 text-muted-foreground">
								{isPrimary ? 'ตั้งเป็นบทบาทหลัก' : 'เพิ่มเป็นบทบาทร่วม'} • {selected?.permissions
									.length ?? 0} สิทธิ์
							</p>
							{#if selected?.description}<p class="mt-2">{selected.description}</p>{/if}
						</div>{/if}
				</div>
			{/if}
			<DialogFooter>
				<Button variant="outline" onclick={() => (showAssignDialog = false)}>ยกเลิก</Button>
				<Button
					onclick={handleAssignRole}
					disabled={assigning || !selectedRoleId || catalogLoading || !!catalogError || !canRead}
				>
					{assigning ? 'กำลังมอบหมาย...' : 'มอบหมาย'}
				</Button>
			</DialogFooter>
		</DialogContent>
	</Dialog>{/if}
