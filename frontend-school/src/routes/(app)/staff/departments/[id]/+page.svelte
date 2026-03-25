<script lang="ts">
	import type { PageProps } from './$types';
	import { onMount } from 'svelte';
	import {
        getDepartment,
        listStaff,
        listDelegations,
        listDelegatablePermissions,
        createDelegation,
        revokeDelegation,
        type Department,
        type StaffListItem,
        type DelegationItem,
        type DelegatablePermission
    } from '$lib/api/staff';
	import { can } from '$lib/stores/permissions';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import {
        Building2, ArrowLeft, Phone, Mail, MapPin,
        Briefcase, GraduationCap, Users, User, Shield, Plus, Trash2
    } from 'lucide-svelte';

	const { params }: PageProps = $props();
	let deptId = $derived(params.id);
	let department: Department | null = $state(null);
	let members: StaffListItem[] = $state([]);
	let delegations: DelegationItem[] = $state([]);
	let delegatablePerms: DelegatablePermission[] = $state([]);
	let loading = $state(true);
	let error = $state('');

	// Delegation form state
	let showDelegateDialog = $state(false);
	let delegateForm = $state({ to_user_id: '', permission_id: '', reason: '', expires_at: '' });
	let delegateSubmitting = $state(false);
	let delegateError = $state('');

	async function loadData() {
        if (!deptId) return;
		try {
			loading = true;

            // Parallel fetch department and staff
            const [deptRes, staffRes] = await Promise.all([
                getDepartment(deptId),
                listStaff({ department_id: deptId, page_size: 100 }) // Fetch up to 100 members first
            ]);

			if (deptRes.success && deptRes.data) {
				department = deptRes.data;
			} else {
                throw new Error(deptRes.error || 'Department not found');
            }

            if (staffRes.success && staffRes.data) {
                members = staffRes.data;
            }

		} catch (e: any) {
			error = e.message || 'Error loading data';
		} finally {
			loading = false;
		}
	}

	async function loadDelegations() {
		if (!deptId) return;
		const [delRes, permRes] = await Promise.all([
			listDelegations(deptId),
			listDelegatablePermissions(deptId)
		]);
		if (delRes.success && delRes.data) delegations = delRes.data;
		if (permRes.success && permRes.data) delegatablePerms = permRes.data;
	}

	async function handleRevoke(delegationId: string) {
		const res = await revokeDelegation(delegationId);
		if (res.success) {
			delegations = delegations.filter(d => d.id !== delegationId);
		}
	}

	async function handleDelegate() {
		if (!deptId || !delegateForm.to_user_id || !delegateForm.permission_id) return;
		delegateSubmitting = true;
		delegateError = '';
		try {
			const body: Record<string, string> = {
				to_user_id: delegateForm.to_user_id,
				permission_id: delegateForm.permission_id
			};
			if (delegateForm.reason) body.reason = delegateForm.reason;
			if (delegateForm.expires_at) body.expires_at = new Date(delegateForm.expires_at).toISOString();

			const res = await createDelegation(deptId, body as any);
			if (res.success) {
				showDelegateDialog = false;
				delegateForm = { to_user_id: '', permission_id: '', reason: '', expires_at: '' };
				await loadDelegations();
			} else {
				delegateError = res.error || 'เกิดข้อผิดพลาด';
			}
		} finally {
			delegateSubmitting = false;
		}
	}

	onMount(() => {
		loadData();
	});

	$effect(() => {
		if (!loading && $can.has('dept_work.approve.department') && deptId) {
			loadDelegations();
		}
	});
</script>

<svelte:head>
	<title>{department ? department.name : 'รายละเอียดฝ่าย'} - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header / Back -->
	<div class="flex items-center gap-4">
		<Button href="/staff/departments" variant="ghost" size="sm">
			<ArrowLeft class="w-4 h-4" />
		</Button>
		<div class="flex-1">
			<h1 class="text-2xl font-bold text-foreground flex items-center gap-2">
				{#if loading}
					กำลังโหลด...
				{:else if department}
					{#if department.category === 'academic'}
						<GraduationCap class="w-8 h-8 text-orange-500" />
					{:else}
						<Briefcase class="w-8 h-8 text-blue-500" />
					{/if}
					{department.name}
				{:else}
					ไม่พบข้อมูล
				{/if}
			</h1>
			{#if department?.name_en}
				<p class="text-muted-foreground ml-10">{department.name_en}</p>
			{/if}
		</div>
	</div>

	{#if loading}
		<div class="p-12 text-center text-muted-foreground">กำลังโหลดข้อมูล...</div>
	{:else if error}
		<div class="p-6 bg-destructive/10 text-destructive rounded-lg">{error}</div>
	{:else if department}
		<!-- Info Cards -->
		<div class="grid grid-cols-1 md:grid-cols-3 gap-6">
			<!-- Left Column: Details -->
			<div class="md:col-span-2 space-y-6">
				<!-- Basic Info -->
				<div class="bg-card border border-border rounded-lg p-6 space-y-4">
					<h2 class="text-lg font-semibold flex items-center gap-2">
						<Building2 class="w-5 h-5" />
						ข้อมูลทั่วไป
					</h2>

					<div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
						<div>
							<span class="text-sm text-muted-foreground">รหัสฝ่าย</span>
							<p class="font-medium">{department.code}</p>
						</div>
						<div>
							<span class="text-sm text-muted-foreground">ประเภทองค์กร</span>
							<div class="flex items-center gap-2 mt-1">
								<Badge variant="outline"
									>{department.category === 'academic' ? 'วิชาการ' : 'บริหารจัดการ'}</Badge
								>
								<Badge variant={department.org_type === 'group' ? 'default' : 'secondary'}>
									{department.org_type === 'group' ? 'กลุ่ม (Group)' : 'หน่วยงาน (Unit)'}
								</Badge>
							</div>
						</div>
						<div class="col-span-2">
							<span class="text-sm text-muted-foreground">รายละเอียด</span>
							<p class="mt-1">{department.description || '-'}</p>
						</div>
					</div>
				</div>

				<!-- Contact Info -->
				<div class="bg-card border border-border rounded-lg p-6 space-y-4">
					<h2 class="text-lg font-semibold flex items-center gap-2">
						<Phone class="w-5 h-5" />
						การติดต่อ
					</h2>
					<div class="grid grid-cols-1 sm:grid-cols-3 gap-4">
						<div class="flex items-center gap-3">
							<div class="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
								<Phone class="w-5 h-5 text-primary" />
							</div>
							<div>
								<span class="text-xs text-muted-foreground block">เบอร์โทรศัพท์</span>
								<span class="text-sm font-medium">{department.phone || '-'}</span>
							</div>
						</div>
						<div class="flex items-center gap-3">
							<div class="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
								<Mail class="w-5 h-5 text-primary" />
							</div>
							<div>
								<span class="text-xs text-muted-foreground block">อีเมล</span>
								<span class="text-sm font-medium">{department.email || '-'}</span>
							</div>
						</div>
						<div class="flex items-center gap-3">
							<div class="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
								<MapPin class="w-5 h-5 text-primary" />
							</div>
							<div>
								<span class="text-xs text-muted-foreground block">สถานที่ตั้ง</span>
								<span class="text-sm font-medium">{department.location || '-'}</span>
							</div>
						</div>
					</div>
				</div>

				<!-- Delegation Section (heads only) -->
				{#if $can.has('dept_work.approve.department')}
					<div class="bg-card border border-border rounded-lg p-6 space-y-4">
						<div class="flex items-center justify-between">
							<h2 class="text-lg font-semibold flex items-center gap-2">
								<Shield class="w-5 h-5" />
								การมอบหมายสิทธิ์
							</h2>
							<Button size="sm" onclick={() => (showDelegateDialog = true)}>
								<Plus class="w-4 h-4 mr-1" />
								มอบหมายสิทธิ์
							</Button>
						</div>

						{#if delegations.length === 0}
							<p class="text-muted-foreground text-sm text-center py-4">ยังไม่มีการมอบหมายสิทธิ์</p>
						{:else}
							<div class="divide-y divide-border">
								{#each delegations as d}
									<div class="py-3 flex items-start justify-between gap-4">
										<div class="space-y-0.5">
											<p class="text-sm font-medium">{d.to_user_name}</p>
											<p class="text-xs text-muted-foreground">{d.permission_name} <span class="font-mono">({d.permission_code})</span></p>
											{#if d.reason}
												<p class="text-xs text-muted-foreground">เหตุผล: {d.reason}</p>
											{/if}
											{#if d.expires_at}
												<p class="text-xs text-muted-foreground">
													หมดอายุ: {new Date(d.expires_at).toLocaleDateString('th-TH')}
												</p>
											{/if}
										</div>
										<Button
											variant="ghost"
											size="sm"
											onclick={() => handleRevoke(d.id)}
											class="text-destructive hover:text-destructive shrink-0"
										>
											<Trash2 class="w-4 h-4" />
										</Button>
									</div>
								{/each}
							</div>
						{/if}
					</div>
				{/if}
			</div>

			<!-- Right Column: Stats / Actions -->
			<div class="space-y-6">
				<div class="bg-card border border-border rounded-lg p-6">
					<h2 class="text-lg font-semibold mb-4 flex items-center gap-2">
						<Users class="w-5 h-5" />
						บุคลากร ({members.length})
					</h2>

					{#if members.length === 0}
						<p class="text-muted-foreground text-center py-8">ไม่มีบุคลากรในฝ่ายนี้</p>
					{:else}
						<div class="space-y-3 max-h-[400px] overflow-y-auto pr-2">
							{#each members as member}
								<a
									href="/staff/manage/{member.id}"
									class="flex items-center gap-3 p-3 rounded-lg hover:bg-muted/50 transition-colors"
								>
									<div
										class="w-10 h-10 rounded-full bg-muted flex items-center justify-center overflow-hidden border border-border"
									>
										<User class="w-5 h-5 text-muted-foreground" />
									</div>
									<div>
										<p class="font-medium text-sm">
											{member.title}{member.first_name}
											{member.last_name}
										</p>
										<p class="text-xs text-muted-foreground">{member.roles.join(', ')}</p>
									</div>
								</a>
							{/each}
						</div>
					{/if}
				</div>
			</div>
		</div>
	{/if}
</div>

<!-- Delegate Permission Dialog -->
{#if showDelegateDialog}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
		<div class="bg-background border border-border rounded-xl shadow-lg w-full max-w-md p-6 space-y-4">
			<h3 class="text-lg font-semibold">มอบหมายสิทธิ์</h3>

			{#if delegateError}
				<div class="text-sm text-destructive bg-destructive/10 rounded p-3">{delegateError}</div>
			{/if}

			<div class="space-y-3">
				<div class="space-y-1">
					<label for="delegate-to" class="text-sm font-medium">สมาชิกที่จะมอบหมายให้ *</label>
					<select
						id="delegate-to"
						bind:value={delegateForm.to_user_id}
						class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
					>
						<option value="">-- เลือกสมาชิก --</option>
						{#each members as m}
							<option value={m.id}>{m.title}{m.first_name} {m.last_name}</option>
						{/each}
					</select>
				</div>

				<div class="space-y-1">
					<label for="delegate-permission" class="text-sm font-medium">สิทธิ์ที่มอบหมาย *</label>
					<select
						id="delegate-permission"
						bind:value={delegateForm.permission_id}
						class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
					>
						<option value="">-- เลือกสิทธิ์ --</option>
						{#each delegatablePerms as p}
							<option value={p.id}>{p.name} ({p.code})</option>
						{/each}
					</select>
				</div>

				<div class="space-y-1">
					<label for="delegate-reason" class="text-sm font-medium">เหตุผล <span class="text-muted-foreground font-normal">(ไม่บังคับ)</span></label>
					<input
						id="delegate-reason"
						type="text"
						bind:value={delegateForm.reason}
						placeholder="เช่น ลาพักร้อน, รักษาการ"
						class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
					/>
				</div>

				<div class="space-y-1">
					<label for="delegate-expires" class="text-sm font-medium">วันหมดอายุ <span class="text-muted-foreground font-normal">(ไม่บังคับ)</span></label>
					<input
						id="delegate-expires"
						type="date"
						bind:value={delegateForm.expires_at}
						class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
					/>
				</div>
			</div>

			<div class="flex justify-end gap-2 pt-2">
				<Button
					variant="outline"
					onclick={() => {
						showDelegateDialog = false;
						delegateError = '';
					}}
				>
					ยกเลิก
				</Button>
				<Button
					onclick={handleDelegate}
					disabled={delegateSubmitting || !delegateForm.to_user_id || !delegateForm.permission_id}
				>
					{delegateSubmitting ? 'กำลังบันทึก...' : 'มอบหมาย'}
				</Button>
			</div>
		</div>
	</div>
{/if}
