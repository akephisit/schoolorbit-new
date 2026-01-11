<script lang="ts">
	import { onMount } from 'svelte';
    import { authStore } from '$lib/stores/auth';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
    import * as Tabs from "$lib/components/ui/tabs";
	import {
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow
	} from '$lib/components/ui/table';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { 
        Award, 
        Plus, 
        Search, 
        Calendar, 
        FileText, 
        User as UserIcon,
        Trash2,
        Pencil,
        ExternalLink,
        LoaderCircle 
    } from 'lucide-svelte';
	import { getAchievements, createAchievement, updateAchievement, deleteAchievement } from '$lib/api/achievement';
	import type { Achievement } from '$lib/types/achievement';
    import AchievementDialog from '$lib/components/achievement/AchievementDialog.svelte';
	import { toast } from 'svelte-sonner';

    // State
	let loading = $state(false);
	let achievements = $state<Achievement[]>([]);
    let searchTerm = $state('');
    let activeTab = $state('own'); // 'own' | 'all'

    // Dialog State
    let showDialog = $state(false);
    let selectedAchievement = $state<Achievement | null>(null);

    // Permissions
    const user = $derived($authStore.user);
    const userId = $derived(user?.id || '');
    // Check permissions from authStore (assuming user object has fully populated permissions or we check roles/scopes)
    // Note: In this system, permissions are strings like 'achievement.read.all'
    const permissions = $derived(user?.permissions || []); 

    const canReadAll = $derived(permissions.includes('achievement.read.all'));
    // const canReadOwn = $derived(permissions.includes('achievement.read.own')); // Implicitly true for valid users usually
    const canCreateAll = $derived(permissions.includes('achievement.create.all'));
    const canCreateOwn = $derived(permissions.includes('achievement.create.own'));
    const canUpdateAll = $derived(permissions.includes('achievement.update.all'));
    const canUpdateOwn = $derived(permissions.includes('achievement.update.own'));
    const canDeleteAll = $derived(permissions.includes('achievement.delete.all'));
    const canDeleteOwn = $derived(permissions.includes('achievement.delete.own'));

    // Derived state for filtering
    const filteredAchievements = $derived(
        achievements.filter(a => {
            const staffName = `${a.user_first_name || ''} ${a.user_last_name || ''}`;
            const searchLower = searchTerm.toLowerCase();
            
            return (
                a.title.toLowerCase().includes(searchLower) ||
                (a.description || '').toLowerCase().includes(searchLower) ||
                staffName.toLowerCase().includes(searchLower)
            );
        })
    );

    // Effect to set default tab based on permission
    $effect(() => {
        if (canReadAll && activeTab === 'own' && achievements.length === 0 && !loading) {
            // If user can read all, maybe we want to default to 'all'? 
            // Or let them switch. For now, default is 'own' but valid to switch.
        }
    });

	async function loadData() {
        if (!userId) return;

		try {
			loading = true;
            
            const filter: any = {};
            if (activeTab === 'own') {
                filter.user_id = userId;
            } 
            // If 'all', send no user_id filter (backend handles permission check too)

			const res = await getAchievements(filter); 
			if (res.success && res.data) {
				achievements = res.data;
			} else {
                achievements = [];
            }
		} catch (error) {
			console.error('Failed to load data:', error);
			toast.error('ไม่สามารถโหลดข้อมูลได้');
		} finally {
			loading = false;
		}
	}

    function handleTabChange(value: string) {
        activeTab = value;
        loadData();
    }

    function formatDate(dateStr: string) {
        return new Date(dateStr).toLocaleDateString('th-TH', {
            year: 'numeric',
            month: 'long',
            day: 'numeric'
        });
    }

    // Actions
    function openCreateDialog() {
        selectedAchievement = null;
        showDialog = true;
    }

    function openEditDialog(achievement: Achievement) {
        selectedAchievement = achievement;
        showDialog = true;
    }

    async function handleSave(e: CustomEvent) {
        const payload = e.detail;
        
        // If create mode and canCreateAll -> payload.user_id might be set to selected user
        // If edit mode -> payload.id exists

        let res;
        if (payload.id) {
            res = await updateAchievement(payload.id, {
                title: payload.title,
                description: payload.description,
                achievement_date: payload.achievement_date,
                image_path: payload.image_path,
                // user_id is generally not updatable via this specific simple DTO but let's check
            });
        } else {
             res = await createAchievement({
                user_id: payload.user_id || userId,
                title: payload.title,
                description: payload.description,
                achievement_date: payload.achievement_date,
                image_path: payload.image_path
            });
        }

        if (res.success) {
            toast.success('บันทึกข้อมูลเรียบร้อย');
            showDialog = false;
            loadData();
        } else {
             toast.error(res.error || 'บันทึกข้อมูลไม่สำเร็จ');
        }
    }

    async function handleDelete(id: string) {
        if (!confirm('คุณต้องการลบรายการนี้ใช่หรือไม่?')) return;
        
        const res = await deleteAchievement(id);
        if (res.success) {
            toast.success('ลบข้อมูลเรียบร้อย');
            achievements = achievements.filter(a => a.id !== id);
        } else {
            toast.error(res.error || 'ลบข้อมูลไม่สำเร็จ');
        }
    }

	onMount(() => {
        // Wait for authStore to be ready (usually immediate in client)
        if (userId) loadData();
	});

    // React to user ID change (e.g. reload or login)
    $effect(() => {
        if (userId && achievements.length === 0 && !loading) {
             loadData();
        }
    });
</script>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-3xl font-bold flex items-center gap-2">
				<Award class="w-8 h-8 text-primary" />
				จัดการข้อมูลเกียรติบัตร
			</h1>
			<p class="text-muted-foreground mt-1">
				บันทึกและจัดการข้อมูลผลงาน รางวัล และเกียรติบัตรของบุคลากร
			</p>
		</div>
		{#if canCreateOwn || canCreateAll}
			<Button onclick={openCreateDialog}>
				<Plus class="w-4 h-4 mr-2" />
				เพิ่มรายการใหม่
			</Button>
		{/if}
	</div>

	{#if canReadAll}
		<Tabs.Root value={activeTab} onValueChange={handleTabChange} class="w-full">
			<Tabs.List class="grid w-full grid-cols-2 max-w-[400px] mb-4">
				<Tabs.Trigger value="own">ของฉัน</Tabs.Trigger>
				<Tabs.Trigger value="all">ภาพรวม (ทั้งหมด)</Tabs.Trigger>
			</Tabs.List>
		</Tabs.Root>
	{/if}

	<Card>
		<CardHeader>
			<CardTitle>รายการผลงาน{activeTab === 'all' ? 'ทั้งหมด' : 'ของฉัน'}</CardTitle>
			<CardDescription
				>แสดงรายการผลงาน{activeTab === 'all'
					? 'ของบุคลากรในระบบ'
					: 'ที่คุณบันทึกไว้'}</CardDescription
			>
		</CardHeader>
		<CardContent>
			<!-- Search -->
			<div class="flex items-center gap-2 mb-6">
				<div class="relative flex-1 max-w-sm">
					<Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
					<Input
						type="search"
						placeholder="ค้นหาตามชื่อผลงาน หรือชื่อเจ้าของ..."
						class="pl-9"
						bind:value={searchTerm}
					/>
				</div>
			</div>

			<!-- Table -->
			<div class="rounded-md border">
				<Table>
					<TableHeader>
						<TableRow>
							<TableHead>วันที่ได้รับ</TableHead>
							<TableHead>ชื่อผลงาน / รางวัล</TableHead>
							<TableHead>เจ้าของผลงาน</TableHead>
							<TableHead>หลักฐาน</TableHead>
							<TableHead class="text-right">จัดการ</TableHead>
						</TableRow>
					</TableHeader>
					<TableBody>
						{#if loading}
							<TableRow>
								<TableCell colspan={5} class="h-24 text-center">
									<div class="flex justify-center items-center gap-2 text-muted-foreground">
										<LoaderCircle class="w-4 h-4 animate-spin" />
										กำลังโหลดข้อมูล...
									</div>
								</TableCell>
							</TableRow>
						{:else if filteredAchievements.length === 0}
							<TableRow>
								<TableCell colspan={5} class="h-24 text-center text-muted-foreground">
									ไม่พบข้อมูล
								</TableCell>
							</TableRow>
						{:else}
							{#each filteredAchievements as achievement (achievement.id)}
								<TableRow>
									<TableCell class="font-medium whitespace-nowrap">
										<div class="flex items-center gap-2">
											<Calendar class="w-4 h-4 text-muted-foreground" />
											{formatDate(achievement.achievement_date)}
										</div>
									</TableCell>
									<TableCell>
										<div class="font-medium">{achievement.title}</div>
										{#if achievement.description}
											<div class="text-xs text-muted-foreground line-clamp-1">
												{achievement.description}
											</div>
										{/if}
									</TableCell>
									<TableCell>
										{#if achievement.user_first_name}
											<div class="flex items-center gap-2">
												<UserIcon class="w-4 h-4 text-muted-foreground" />
												<span>
													{achievement.user_first_name}
													{achievement.user_last_name}
												</span>
											</div>
										{:else}
											<span class="text-muted-foreground text-xs">Unknown User</span>
										{/if}
									</TableCell>
									<TableCell>
										{#if achievement.image_path}
											<a
												href={achievement.image_path.startsWith('http')
													? achievement.image_path
													: `/api/files?path=${achievement.image_path}`}
												target="_blank"
												class="flex items-center gap-1 text-primary hover:underline text-sm"
											>
												<FileText class="w-4 h-4" />
												ดูไฟล์
											</a>
										{:else}
											<span class="text-muted-foreground text-xs">-</span>
										{/if}
									</TableCell>
									<TableCell class="text-right">
										<div class="flex justify-end gap-2">
											{#if canReadAll || canUpdateAll}
												<Button
													variant="ghost"
													size="icon"
													class="h-8 w-8"
													href={`/staff/manage/${achievement.user_id}`}
													title="ดูโปรไฟล์"
												>
													<ExternalLink class="w-4 h-4" />
												</Button>
											{/if}

											{#if canUpdateAll || (canUpdateOwn && achievement.user_id === userId)}
												<Button
													variant="ghost"
													size="icon"
													class="h-8 w-8 hover:bg-muted"
													onclick={() => openEditDialog(achievement)}
													title="แก้ไข"
												>
													<Pencil class="w-4 h-4" />
												</Button>
											{/if}

											{#if canDeleteAll || (canDeleteOwn && achievement.user_id === userId)}
												<Button
													variant="ghost"
													size="icon"
													class="h-8 w-8 text-destructive hover:text-destructive hover:bg-destructive/10"
													onclick={() => handleDelete(achievement.id)}
													title="ลบ"
												>
													<Trash2 class="w-4 h-4" />
												</Button>
											{/if}
										</div>
									</TableCell>
								</TableRow>
							{/each}
						{/if}
					</TableBody>
				</Table>
			</div>
		</CardContent>
	</Card>

	<AchievementDialog
		open={showDialog}
		achievement={selectedAchievement}
		{userId}
		canSelectUser={canCreateAll}
		on:close={() => (showDialog = false)}
		on:save={handleSave}
	/>

	<Card>
		<CardHeader>
			<CardTitle>รายการผลงานทั้งหมด</CardTitle>
			<CardDescription>แสดงรายการผลงานของบุคลากรในระบบ</CardDescription>
		</CardHeader>
		<CardContent>
			<!-- Search -->
			<div class="flex items-center gap-2 mb-6">
				<div class="relative flex-1 max-w-sm">
					<Search class="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
					<Input
						type="search"
						placeholder="ค้นหาตามชื่อผลงาน หรือชื่อเจ้าของ..."
						class="pl-9"
						bind:value={searchTerm}
					/>
				</div>
			</div>

			<!-- Table -->
			<div class="rounded-md border">
				<Table>
					<TableHeader>
						<TableRow>
							<TableHead>วันที่ได้รับ</TableHead>
							<TableHead>ชื่อผลงาน / รางวัล</TableHead>
							<TableHead>เจ้าของผลงาน</TableHead>
							<TableHead>หลักฐาน</TableHead>
							<TableHead class="text-right">จัดการ</TableHead>
						</TableRow>
					</TableHeader>
					<TableBody>
						{#if loading}
							<TableRow>
								<TableCell colspan={5} class="h-24 text-center">
									<div class="flex justify-center items-center gap-2 text-muted-foreground">
										<LoaderCircle class="w-4 h-4 animate-spin" />
										กำลังโหลดข้อมูล...
									</div>
								</TableCell>
							</TableRow>
						{:else if filteredAchievements.length === 0}
							<TableRow>
								<TableCell colspan={5} class="h-24 text-center text-muted-foreground">
									ไม่พบข้อมูล
								</TableCell>
							</TableRow>
						{:else}
							{#each filteredAchievements as achievement (achievement.id)}
								<TableRow>
									<TableCell class="font-medium whitespace-nowrap">
										<div class="flex items-center gap-2">
											<Calendar class="w-4 h-4 text-muted-foreground" />
											{formatDate(achievement.achievement_date)}
										</div>
									</TableCell>
									<TableCell>
										<div class="font-medium">{achievement.title}</div>
										{#if achievement.description}
											<div class="text-xs text-muted-foreground line-clamp-1">
												{achievement.description}
											</div>
										{/if}
									</TableCell>
									<TableCell>
										{#if achievement.user_first_name}
											<div class="flex items-center gap-2">
												<UserIcon class="w-4 h-4 text-muted-foreground" />
												<span>
													{achievement.user_first_name}
													{achievement.user_last_name}
												</span>
											</div>
										{:else}
											<span class="text-muted-foreground text-xs">Unknown User</span>
										{/if}
									</TableCell>
									<TableCell>
										{#if achievement.image_path}
											<a
												href={achievement.image_path.startsWith('http')
													? achievement.image_path
													: `/api/files?path=${achievement.image_path}`}
												target="_blank"
												class="flex items-center gap-1 text-primary hover:underline text-sm"
											>
												<FileText class="w-4 h-4" />
												ดูไฟล์
											</a>
										{:else}
											<span class="text-muted-foreground text-xs">-</span>
										{/if}
									</TableCell>
									<TableCell class="text-right">
										<div class="flex justify-end gap-2">
											<Button
												variant="ghost"
												size="icon"
												class="h-8 w-8"
												href={`/staff/manage/${achievement.user_id}`}
											>
												<ExternalLink class="w-4 h-4" />
											</Button>
											<Button
												variant="ghost"
												size="icon"
												class="h-8 w-8 text-destructive hover:text-destructive hover:bg-destructive/10"
												onclick={() => handleDelete(achievement.id)}
											>
												<Trash2 class="w-4 h-4" />
											</Button>
										</div>
									</TableCell>
								</TableRow>
							{/each}
						{/if}
					</TableBody>
				</Table>
			</div>
		</CardContent>
	</Card>
</div>
