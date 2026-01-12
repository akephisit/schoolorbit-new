<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { getStaffProfile, type StaffProfileResponse } from '$lib/api/staff';
	import { Button } from '$lib/components/ui/button';
	import {
		User,
		Mail,
		Phone,
		Briefcase,
		GraduationCap,
		Building2,
		BookOpen,
		ArrowLeft,
		Pencil,
		Award,
		Plus
	} from 'lucide-svelte';
	import type { Achievement } from '$lib/types/achievement';
	import { getAchievements, createAchievement, updateAchievement, deleteAchievement } from '$lib/api/achievement';
	import AchievementCard from '$lib/components/achievement/AchievementCard.svelte';
	import AchievementDialog from '$lib/components/achievement/AchievementDialog.svelte';
	import { toast } from 'svelte-sonner';

	let staff: StaffProfileResponse | null = $state(null);
	let loading = $state(true);
	let error = $state('');
    
    // Achievement State
	let achievements: Achievement[] = $state([]);
	let loadingAchievements = $state(false);
	let showAchievementDialog = $state(false);
	let selectedAchievement: Achievement | null = $state(null);
    let showDeleteDialog = $state(false);
    let deleteId = $state<string | null>(null);

	const staffId = $derived(page.params.id);

	async function loadStaffProfile() {
		if (!staffId) return;

		try {
			loading = true;
			error = '';
			const response = await getStaffProfile(staffId);
			if (response.success && response.data) {
				staff = response.data;
				// Load achievements after staff is loaded
				loadAchievements();
			} else {
				error = response.error || 'ไม่พบข้อมูล';
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'เกิดข้อผิดพลาด';
			console.error('Failed to load staff profile:', e);
		} finally {
			loading = false;
		}
	}

	async function loadAchievements() {
		if (!staffId) return;
		loadingAchievements = true;
		const res = await getAchievements({ user_id: staffId });
		if (res.success && res.data) {
			achievements = res.data;
		}
		loadingAchievements = false;
	}

	async function handleSaveAchievement(payload: any) {
		let res;
		if (payload.id) {
			res = await updateAchievement(payload.id, {
				title: payload.title,
				description: payload.description,
				achievement_date: payload.achievement_date,
				image_path: payload.image_path
			});
		} else {
			res = await createAchievement({
				user_id: payload.user_id,
				title: payload.title,
				description: payload.description,
				achievement_date: payload.achievement_date,
				image_path: payload.image_path
			});
		}

		if (res.success) {
			toast.success(payload.id ? 'แก้ไขผลงานเรียบร้อย' : 'เพิ่มผลงานเรียบร้อย');
			showAchievementDialog = false;
			loadAchievements();
		} else {
			toast.error(res.error || 'เกิดข้อผิดพลาด');
		}
	}

	function confirmDelete(achievement: Achievement) {
        deleteId = achievement.id;
        showDeleteDialog = true;
	}

    async function handleConfirmDelete() {
        if (!deleteId) return;
        
        const res = await deleteAchievement(deleteId);
        if (res.success) {
            toast.success('ลบผลงานเรียบร้อย');
            loadAchievements();
        } else {
            toast.error(res.error || 'เกิดข้อผิดพลาด');
        }
        showDeleteDialog = false;
        deleteId = null;
    }

	onMount(() => {
		loadStaffProfile();
	});
</script>

<svelte:head>
	<title>
		{staff ? `${staff.first_name} ${staff.last_name}` : 'ข้อมูลบุคลากร'} - SchoolOrbit
	</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-4">
			<Button href="/staff" variant="ghost" size="sm">
				<ArrowLeft class="w-4 h-4" />
			</Button>
			<div>
				<h1 class="text-3xl font-bold text-foreground">ข้อมูลบุคลากร</h1>
				<p class="text-muted-foreground mt-1">รายละเอียดบุคลากร</p>
			</div>
		</div>
		{#if staff}
			<Button href="/staff/manage/{staff.id}/edit" class="flex items-center gap-2">
				<Pencil class="w-4 h-4" />
				แก้ไข
			</Button>
		{/if}
	</div>

	{#if loading}
		<div class="bg-card border border-border rounded-lg p-12 text-center">
			<div
				class="inline-block w-8 h-8 border-4 border-primary border-t-transparent rounded-full animate-spin"
			></div>
			<p class="mt-4 text-muted-foreground">กำลังโหลด...</p>
		</div>
	{:else if error}
		<div class="bg-destructive/10 border border-destructive/20 rounded-lg p-6 text-center">
			<p class="text-destructive">{error}</p>
			<Button onclick={loadStaffProfile} variant="outline" class="mt-4">ลองอีกครั้ง</Button>
		</div>
	{:else if staff}
		<!-- Profile Card -->
		<div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
			<!-- Left Column - Basic Info -->
			<div class="lg:col-span-1 space-y-6">
				<!-- Profile Card -->
				<div class="bg-card border border-border rounded-lg p-6">
					<div class="text-center">
						<div
							class="w-24 h-24 rounded-full bg-primary/10 flex items-center justify-center mx-auto mb-4 overflow-hidden"
						>
							{#if staff.profile_image_url}
								<img
									src={staff.profile_image_url}
									alt={`${staff.first_name} ${staff.last_name}`}
									class="w-full h-full object-cover"
								/>
							{:else}
								<User class="w-12 h-12 text-primary" />
							{/if}
						</div>
						<h2 class="text-2xl font-bold text-foreground">
							{staff.title || ''}
							{staff.first_name}
							{staff.last_name}
						</h2>
						{#if staff.nickname}
							<p class="text-muted-foreground mt-1">({staff.nickname})</p>
						{/if}

						<!-- Status Badge -->
						<div class="mt-4">
							{#if staff.status === 'active'}
								<span
									class="inline-flex items-center px-3 py-1 bg-green-100 text-green-800 rounded-full text-sm"
								>
									<span class="w-2 h-2 rounded-full bg-green-500 mr-2"></span>
									ใช้งาน
								</span>
							{:else}
								<span
									class="inline-flex items-center px-3 py-1 bg-gray-100 text-gray-800 rounded-full text-sm"
								>
									<span class="w-2 h-2 rounded-full bg-gray-500 mr-2"></span>
									ไม่ใช้งาน
								</span>
							{/if}
						</div>
					</div>

					<div class="mt-6 space-y-3 border-t border-border pt-6">
						{#if staff.email}
							<div class="flex items-center gap-3 text-sm">
								<Mail class="w-4 h-4 text-muted-foreground" />
								<span class="text-foreground">{staff.email}</span>
							</div>
						{/if}
						{#if staff.phone}
							<div class="flex items-center gap-3 text-sm">
								<Phone class="w-4 h-4 text-muted-foreground" />
								<span class="text-foreground">{staff.phone}</span>
							</div>
						{/if}
						{#if staff.national_id}
							<div class="flex items-center gap-3 text-sm">
								<User class="w-4 h-4 text-muted-foreground" />
								<span class="text-foreground">บัตรปชช.: {staff.national_id}</span>
							</div>
						{/if}
					</div>
				</div>

				<!-- Staff Info Card -->
				{#if staff.staff_info}
					<div class="bg-card border border-border rounded-lg p-6">
						<h3 class="font-semibold text-foreground mb-4 flex items-center gap-2">
							<Briefcase class="w-5 h-5" />
							ข้อมูลการทำงาน
						</h3>
						<div class="space-y-3 text-sm">
							{#if staff.staff_info.education_level}
								<div>
									<p class="text-muted-foreground">วุฒิการศึกษา</p>
									<p class="text-foreground font-medium">{staff.staff_info.education_level}</p>
								</div>
							{/if}
							{#if staff.staff_info.major}
								<div>
									<p class="text-muted-foreground">สาขา</p>
									<p class="text-foreground font-medium">{staff.staff_info.major}</p>
								</div>
							{/if}
							{#if staff.staff_info.university}
								<div>
									<p class="text-muted-foreground">สถาบัน</p>
									<p class="text-foreground font-medium">{staff.staff_info.university}</p>
								</div>
							{/if}
						</div>
					</div>
				{/if}
			</div>

			<!-- Right Column - Details -->
			<div class="lg:col-span-2 space-y-6">
				<!-- Roles Card -->
				<div class="bg-card border border-border rounded-lg p-6">
					<h3 class="font-semibold text-foreground mb-4 flex items-center gap-2">
						<GraduationCap class="w-5 h-5" />
						บทบาทและตำแหน่ง
					</h3>
					{#if staff.roles.length > 0}
						<div class="flex flex-wrap gap-2">
							{#each staff.roles as role (role.id)}
								<div
									class="px-4 py-2 rounded-lg border border-border {role.is_primary
										? 'bg-primary/10 border-primary'
										: 'bg-muted'}"
								>
									<div class="flex items-center gap-2">
										<span class="font-medium text-foreground">{role.name}</span>
										{#if role.is_primary}
											<span
												class="text-xs px-2 py-0.5 bg-primary text-primary-foreground rounded-full"
											>
												หลัก
											</span>
										{/if}
									</div>
									<p class="text-xs text-muted-foreground mt-1">
										{role.user_type} • ระดับ {role.level}
									</p>
								</div>
							{/each}
						</div>
					{:else}
						<p class="text-muted-foreground">ยังไม่มีบทบาท</p>
					{/if}
				</div>

				<!-- Departments Card -->
				<div class="bg-card border border-border rounded-lg p-6">
					<h3 class="font-semibold text-foreground mb-4 flex items-center gap-2">
						<Building2 class="w-5 h-5" />
						สังกัดฝ่าย
					</h3>
					{#if staff.departments.length > 0}
						<div class="space-y-3">
							{#each staff.departments as dept (dept.id)}
								<div
									class="px-4 py-3 rounded-lg border border-border {dept.is_primary_department
										? 'bg-primary/5 border-primary/30'
										: 'bg-muted/50'}"
								>
									<div class="flex items-start justify-between">
										<div>
											<p class="font-medium text-foreground">{dept.name}</p>
											<p class="text-sm text-muted-foreground mt-1">
												{dept.position || 'สมาชิก'}
											</p>
										</div>
										{#if dept.is_primary_department}
											<span
												class="text-xs px-2 py-1 bg-primary text-primary-foreground rounded-full"
											>
												ฝ่ายหลัก
											</span>
										{/if}
									</div>
								</div>
							{/each}
						</div>
					{:else}
						<p class="text-muted-foreground">ยังไม่ได้สังกัดฝ่าย</p>
					{/if}
				</div>

				<!-- Teaching Assignments Card -->
				<div class="bg-card border border-border rounded-lg p-6">
					<h3 class="font-semibold text-foreground mb-4 flex items-center gap-2">
						<BookOpen class="w-5 h-5" />
						วิชาที่สอน
					</h3>
					{#if staff.teaching_assignments.length > 0}
						<div class="space-y-3">
							{#each staff.teaching_assignments as assignment (assignment.id)}
								<div class="px-4 py-3 rounded-lg bg-muted/50 border border-border">
									<div class="flex items-start justify-between">
										<div>
											<p class="font-medium text-foreground">{assignment.subject}</p>
											<p class="text-sm text-muted-foreground mt-1">
												{#if assignment.class_name}
													{assignment.class_name}
													{#if assignment.grade_level}
														• {assignment.grade_level}
													{/if}
												{:else if assignment.grade_level}
													{assignment.grade_level}
												{/if}
												{#if assignment.hours_per_week}
													• {assignment.hours_per_week} ชม./สัปดาห์
												{/if}
											</p>
											<p class="text-xs text-muted-foreground mt-1">
												ปีการศึกษา {assignment.academic_year} เทอม {assignment.semester}
											</p>
										</div>
										{#if assignment.is_homeroom_teacher}
											<span class="text-xs px-2 py-1 bg-primary/10 text-primary rounded-full">
												ครูที่ปรึกษา
											</span>
										{/if}
									</div>
								</div>
							{/each}
						</div>
					{:else}
						<p class="text-muted-foreground">ยังไม่มีวิชาที่สอน</p>
					{/if}
				</div>
			</div>
		</div>
		<!-- Achievements Card -->
		<div class="bg-card border border-border rounded-lg p-6">
			<div class="flex items-center justify-between mb-4">
				<h3 class="font-semibold text-foreground flex items-center gap-2">
					<Award class="w-5 h-5" />
					ผลงานและรางวัล
				</h3>
				<Button
					variant="outline"
					size="sm"
					class="h-8"
					onclick={() => {
						selectedAchievement = null;
						showAchievementDialog = true;
					}}
				>
					<Plus class="w-4 h-4 mr-2" />
					เพิ่มผลงาน
				</Button>
			</div>

			{#if loadingAchievements}
				<div class="py-8 text-center text-muted-foreground">กำลังโหลดข้อมูล...</div>
			{:else if achievements.length > 0}
				<div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
					{#each achievements as achievement (achievement.id)}
						<AchievementCard
							{achievement}
							on:edit={(e) => {
								selectedAchievement = e.detail;
								showAchievementDialog = true;
							}}
							on:delete={(e) => confirmDelete(e.detail)}
						/>
					{/each}
				</div>
			{:else}
				<div class="py-8 text-center bg-muted/20 rounded-lg border border-dashed">
					<Award class="w-10 h-10 text-muted-foreground/30 mx-auto mb-2" />
					<p class="text-sm text-muted-foreground">ยังไม่มีข้อมูลผลงาน</p>
				</div>
			{/if}
		</div>
	{/if}

	<AchievementDialog
		open={showAchievementDialog}
		achievement={selectedAchievement}
		userId={staffId ?? ''}
		onclose={() => (showAchievementDialog = false)}
		onsave={handleSaveAchievement}
	/>

	<!-- Delete Confirmation Dialog -->
	<Dialog.Root bind:open={showDeleteDialog}>
		<Dialog.Content class="sm:max-w-[425px]">
			<Dialog.Header>
				<Dialog.Title>ยืนยันการลบข้อมูล</Dialog.Title>
				<Dialog.Description>
					คุณต้องการลบรายการนี้ใช่หรือไม่? การกระทำนี้ไม่สามารถย้อนกลับได้
				</Dialog.Description>
			</Dialog.Header>
			<Dialog.Footer>
				<Button variant="outline" onclick={() => (showDeleteDialog = false)}>ยกเลิก</Button>
				<Button variant="destructive" onclick={handleConfirmDelete}>ลบข้อมูล</Button>
			</Dialog.Footer>
		</Dialog.Content>
	</Dialog.Root>
</div>
