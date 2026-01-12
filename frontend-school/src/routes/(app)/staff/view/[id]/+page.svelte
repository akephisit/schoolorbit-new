<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { Button } from '$lib/components/ui/button';
	import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '$lib/components/ui/card';
	import { Avatar } from '$lib/components/ui/avatar';
	import { Badge } from '$lib/components/ui/badge';
    import { Separator } from '$lib/components/ui/separator';
	import { 
        ArrowLeft, 
        Building2, 
        Briefcase, 
        Mail, 
        Phone, 
        Calendar, 
        Award, 
        User,
        FileText
    } from 'lucide-svelte';
	import { getPublicStaffProfile } from '$lib/api/staff';
    import { getAchievements } from '$lib/api/achievement';
	import type { StaffProfileResponse } from '$lib/api/staff';
    import type { Achievement } from '$lib/types/achievement';
	import { toast } from 'svelte-sonner';
    import { LoaderCircle } from 'lucide-svelte';

	const staffId = $page.params.id ?? '';
	let staff = $state<StaffProfileResponse | null>(null);
    let achievements = $state<Achievement[]>([]);
	let loading = $state(true);
    let loadingAchievements = $state(true);

	async function loadStaffProfile() {
		try {
			const res = await getPublicStaffProfile(staffId);
			if (res.success && res.data) {
				staff = res.data;
			} else {
				toast.error('ไม่พบข้อมูลบุคลากร');
				goto('/staff/achievements');
			}
		} catch (error) {
			console.error('Error loading staff:', error);
			toast.error('เกิดข้อผิดพลาดในการโหลดข้อมูล');
		} finally {
			loading = false;
		}
	}

    async function loadAchievements() {
        try {
            const res = await getAchievements({ user_id: staffId });
            if (res.success && res.data) {
                achievements = res.data;
            }
        } catch (error) {
            console.error('Error loading achievements:', error);
        } finally {
            loadingAchievements = false;
        }
    }

    function formatDate(dateStr: string) {
        if (!dateStr) return '-';
        return new Date(dateStr).toLocaleDateString('th-TH', {
            year: 'numeric',
            month: 'long',
            day: 'numeric'
        });
    }

	onMount(() => {
		loadStaffProfile();
        loadAchievements();
	});
</script>

<div class="max-w-7xl mx-auto space-y-6">
	<!-- Back Button -->
	<Button
		variant="ghost"
		onclick={() => history.back()}
		class="mb-4 pl-0 hover:pl-2 transition-all"
	>
		<ArrowLeft class="w-4 h-4 mr-2" />
		ย้อนกลับ
	</Button>

	{#if loading}
		<div class="flex justify-center items-center h-64">
			<LoaderCircle class="w-8 h-8 animate-spin text-primary" />
		</div>
	{:else if staff}
		<!-- Header Profile Card -->
		<Card
			class="overflow-hidden border-none shadow-lg bg-gradient-to-br from-primary/5 via-card to-card"
		>
			<CardContent class="p-8">
				<div class="flex flex-col md:flex-row items-center gap-8">
					<!-- Avatar -->
					<div class="relative group">
						<div
							class="absolute -inset-1 rounded-full bg-gradient-to-r from-primary to-primary/50 opacity-30 blur group-hover:opacity-60 transition-opacity"
						></div>
						<Avatar
							src={staff.profile_image_url}
							alt={staff.first_name}
							initials={staff.first_name[0] + (staff.last_name[0] || '')}
							size="xl"
							class="w-32 h-32 border-4 border-background relative shadow-xl"
						/>
					</div>

					<!-- Basic Info -->
					<div class="text-center md:text-left space-y-2 flex-1">
						<div>
							<h1 class="text-3xl font-bold tracking-tight text-foreground">
								{staff.title
									? `${staff.title}${staff.first_name} ${staff.last_name}`
									: `${staff.first_name} ${staff.last_name}`}
							</h1>
							{#if staff.nickname}
								<p class="text-lg text-muted-foreground font-medium">({staff.nickname})</p>
							{/if}
						</div>

						<div class="flex flex-wrap justify-center md:justify-start gap-2 mt-3">
							{#if staff.roles && staff.roles.length > 0}
								{#each staff.roles as role}
									<Badge
										variant="secondary"
										class="bg-primary/10 text-primary hover:bg-primary/20 border-primary/20"
									>
										<Briefcase class="w-3 h-3 mr-1" />
										{role.name}
									</Badge>
								{/each}
							{/if}

							{#if staff.departments && staff.departments.length > 0}
								{#each staff.departments as dept}
									<Badge variant="outline" class="border-border/60">
										<Building2 class="w-3 h-3 mr-1" />
										{dept.name}
									</Badge>
								{/each}
							{/if}
						</div>
					</div>
				</div>
			</CardContent>
		</Card>

		<div class="grid grid-cols-1 md:grid-cols-3 gap-6">
			<!-- Left Column: Contact & Info -->
			<div class="space-y-6">
				<Card>
					<CardHeader>
						<CardTitle class="text-lg flex items-center gap-2">
							<User class="w-5 h-5 text-primary" />
							ข้อมูลเบื้องต้น
						</CardTitle>
					</CardHeader>
					<CardContent class="space-y-4">
						<div class="space-y-1">
							<span class="text-xs text-muted-foreground uppercase font-semibold">อีเมล</span>
							<div class="flex items-center gap-2 text-sm">
								<Mail class="w-4 h-4 text-muted-foreground" />
								{staff.email || '-'}
							</div>
						</div>

						{#if staff.phone}
							<div class="space-y-1">
								<span class="text-xs text-muted-foreground uppercase font-semibold"
									>เบอร์โทรศัพท์</span
								>
								<div class="flex items-center gap-2 text-sm">
									<Phone class="w-4 h-4 text-muted-foreground" />
									{staff.phone}
								</div>
							</div>
						{/if}

						{#if staff.hired_date}
							<div class="space-y-1">
								<div class="flex items-center justify-between">
									<span class="text-xs text-muted-foreground uppercase font-semibold"
										>เริ่มงานเมื่อ</span
									>
								</div>
								<div class="flex items-center gap-2 text-sm">
									<Calendar class="w-4 h-4 text-muted-foreground" />
									{formatDate(staff.hired_date)}
								</div>
							</div>
						{/if}
					</CardContent>
				</Card>
			</div>

			<!-- Right Column: Achievements -->
			<div class="md:col-span-2 space-y-6">
				<Card class="flex-1 h-full">
					<CardHeader>
						<CardTitle class="text-xl flex items-center gap-2">
							<Award class="w-6 h-6 text-yellow-500" />
							ผลงานและรางวัลที่ได้รับ
						</CardTitle>
						<CardDescription>รายการความภาคภูมิใจและเกียรติบัตรทั้งหมด</CardDescription>
					</CardHeader>
					<CardContent>
						{#if loadingAchievements}
							<div class="flex justify-center py-8">
								<LoaderCircle class="w-6 h-6 animate-spin text-muted-foreground" />
							</div>
						{:else if achievements.length === 0}
							<div
								class="text-center py-8 text-muted-foreground bg-muted/30 rounded-lg border border-dashed"
							>
								<Award class="w-12 h-12 mx-auto mb-2 opacity-20" />
								<p>ยังไม่มีรายการผลงาน</p>
							</div>
						{:else}
							<div
								class="relative space-y-0 before:absolute before:inset-0 before:ml-5 before:-translate-x-px md:before:mx-auto md:before:translate-x-0 before:h-full before:w-0.5 before:bg-gradient-to-b before:from-transparent before:via-border before:to-transparent"
							>
								{#each achievements as item}
									<div
										class="relative flex items-center justify-between md:justify-normal md:odd:flex-row-reverse group is-active mb-8 last:mb-0"
									>
										<!-- Icon -->
										<div
											class="flex items-center justify-center w-10 h-10 rounded-full border border-background bg-background shadow shrink-0 md:order-1 md:group-odd:-translate-x-1/2 md:group-even:translate-x-1/2 z-10"
										>
											<Award class="w-5 h-5 text-yellow-500" />
										</div>

										<!-- Card Content -->
										<div
											class="w-[calc(100%-4rem)] md:w-[calc(50%-2.5rem)] bg-card p-4 rounded-xl border shadow-sm hover:shadow-md transition-shadow"
										>
											<div class="flex items-center justify-between mb-1">
												<time class="font-caveat font-medium text-sm text-primary"
													>{formatDate(item.achievement_date)}</time
												>
											</div>
											<div class="text-base font-bold text-foreground mb-1">{item.title}</div>
											{#if item.description}
												<div class="text-muted-foreground text-sm line-clamp-2 mb-3">
													{item.description}
												</div>
											{/if}

											{#if item.image_path}
												<a
													href={item.image_path.startsWith('http')
														? item.image_path
														: `/api/files?path=${item.image_path}`}
													target="_blank"
													class="inline-flex items-center gap-1.5 text-xs font-medium text-primary hover:text-primary/80 transition-colors bg-primary/5 px-2.5 py-1.5 rounded-md"
												>
													<FileText class="w-3.5 h-3.5" />
													ดูเอกสาร/เกียรติบัตร
												</a>
											{/if}
										</div>
									</div>
								{/each}
							</div>
						{/if}
					</CardContent>
				</Card>
			</div>
		</div>
	{/if}
</div>
