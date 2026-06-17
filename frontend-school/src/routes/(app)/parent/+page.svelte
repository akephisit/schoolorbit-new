<script lang="ts">
	import { onMount } from 'svelte';
	import { getOwnParentProfile, type ParentProfile } from '$lib/api/parents';
	import { Card } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { User, ChevronRight } from 'lucide-svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';

	let profile = $state<ParentProfile | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);

	function goToStudent(id: string) {
		goto(resolve(`/parent/student/${id}`));
	}

	async function loadProfile() {
		loading = true;
		error = null;
		try {
			const response = await getOwnParentProfile();
			profile = response.data;
		} catch (e) {
			console.error('Failed to load profile:', e);
			error = e instanceof Error ? e.message : 'ไม่สามารถโหลดข้อมูลได้';
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		loadProfile();
	});
</script>

<svelte:head>
	<title>ผู้ปกครอง - SchoolOrbit</title>
</svelte:head>

<PageShell
	title={`สวัสดี, คุณ${profile?.first_name || '...'} ${profile?.last_name || ''}`}
	description="ติดตามการเรียนและความเป็นอยู่ของบุตรหลาน"
>
	{#if loading}
		<PageSkeleton variant="cards" rows={3} />
	{:else if error}
		<PageState
			variant="error"
			title="โหลดข้อมูลผู้ปกครองไม่สำเร็จ"
			description={error}
			actionLabel="ลองอีกครั้ง"
			onaction={loadProfile}
		/>
	{:else if profile}
		<!-- Children List -->
		<div>
			<h2 class="text-xl font-semibold mb-4">บุตรหลานของคุณ</h2>

			{#if profile.children.length === 0}
				<PageState
					title="ไม่พบข้อมูลบุตรหลาน"
					description="ยังไม่มีข้อมูลนักเรียนที่เชื่อมโยงกับบัญชีนี้ กรุณาติดต่อทางโรงเรียนหากข้อมูลไม่ถูกต้อง"
				/>
			{:else}
				<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
					{#each profile.children as child (child.id)}
						<Card
							class="overflow-hidden hover:shadow-lg transition-all cursor-pointer group"
							onclick={() => goToStudent(String(child.id))}
						>
							<div class="p-6">
								<div class="flex items-start gap-4">
									<div
										class="w-16 h-16 rounded-full bg-primary/10 flex items-center justify-center overflow-hidden border-2 border-background shadow-sm"
									>
										{#if child.profile_image_url}
											<img
												src={child.profile_image_url}
												alt={child.first_name}
												class="w-full h-full object-cover"
											/>
										{:else}
											<User class="w-8 h-8 text-primary" />
										{/if}
									</div>
									<div class="flex-1 min-w-0">
										<h3
											class="font-semibold text-lg truncate group-hover:text-primary transition-colors"
										>
											{child.first_name}
											{child.last_name}
										</h3>
										<p class="text-sm text-muted-foreground mb-1">
											รหัสนักเรียน: {child.student_id || '-'}
										</p>
										<div class="flex flex-wrap gap-2">
											<Badge variant="secondary" class="font-normal">
												{child.grade_level || 'ไม่ระบุชั้น'}
											</Badge>
											<Badge variant="outline" class="font-normal text-muted-foreground">
												ห้อง {child.class_room || '-'}
											</Badge>
										</div>
									</div>
									<ChevronRight
										class="w-5 h-5 text-muted-foreground/30 group-hover:text-primary transition-colors"
									/>
								</div>
							</div>
							<div class="bg-muted/30 px-6 py-3 border-t flex justify-between items-center">
								<span class="text-xs text-muted-foreground">สถานะ: {child.relationship}</span>
								<span class="text-xs font-medium text-primary flex items-center">
									ดูรายละเอียด
								</span>
							</div>
						</Card>
					{/each}
				</div>
			{/if}
		</div>
	{:else}
		<PageState
			title="ไม่พบข้อมูลผู้ปกครอง"
			description="ไม่พบโปรไฟล์ผู้ปกครองของบัญชีนี้ กรุณาติดต่อผู้ดูแลระบบ"
		/>
	{/if}
</PageShell>
