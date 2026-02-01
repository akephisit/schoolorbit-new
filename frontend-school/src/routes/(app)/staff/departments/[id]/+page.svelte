<script lang="ts">
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { 
        getDepartment, 
        listStaff, 
        type Department, 
        type StaffListItem 
    } from '$lib/api/staff';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { 
        Building2, ArrowLeft, Phone, Mail, MapPin, 
        Briefcase, GraduationCap, Users, User
    } from 'lucide-svelte';

	let deptId = $derived(page.params.id);
	let department: Department | null = $state(null);
	let members: StaffListItem[] = $state([]);
	let loading = $state(true);
	let error = $state('');

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

	onMount(() => {
		loadData();
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
