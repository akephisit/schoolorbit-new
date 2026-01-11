<script lang="ts">
	import { onMount } from 'svelte';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
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
	import { getAchievements, deleteAchievement } from '$lib/api/achievement';
	import type { Achievement } from '$lib/types/achievement';
	import { toast } from 'svelte-sonner';

	let loading = $state(false);
	let achievements = $state<Achievement[]>([]);
    let searchTerm = $state('');

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

	async function loadData() {
		try {
			loading = true;
            
            // Fetch all achievements (backend now includes user details)
			const res = await getAchievements(); 
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

    function formatDate(dateStr: string) {
        return new Date(dateStr).toLocaleDateString('th-TH', {
            year: 'numeric',
            month: 'long',
            day: 'numeric'
        });
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
		loadData();
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
		<!-- 
        <Button href="/staff/achievements/create">
            <Plus class="w-4 h-4 mr-2" />
            เพิ่มรายการใหม่
        </Button>
        -->
	</div>

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
