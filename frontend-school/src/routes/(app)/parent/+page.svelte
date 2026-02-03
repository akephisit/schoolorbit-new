<script lang="ts">
	import { onMount } from 'svelte';
	import { getOwnParentProfile, type ParentProfile } from '$lib/api/parents';
	import { Card } from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import { Avatar } from '$lib/components/ui/avatar';
	import { Badge } from '$lib/components/ui/badge';
	import { Loader2, User, ChevronRight } from 'lucide-svelte';
	import { goto } from '$app/navigation';

	let profile = $state<ParentProfile | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);

	onMount(async () => {
		try {
			const response = await getOwnParentProfile();
			profile = response.data;
		} catch (e) {
			console.error('Failed to load profile:', e);
			error = e instanceof Error ? e.message : 'ไม่สามารถโหลดข้อมูลได้';
		} finally {
			loading = false;
		}
	});
</script>

<svelte:head>
	<title>ผู้ปกครอง - SchoolOrbit</title>
</svelte:head>

<div class="space-y-6">
	<!-- Profile Header -->
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-3xl font-bold tracking-tight">
				สวัสดี, คุณ{profile?.first_name || '...'}
				{profile?.last_name || ''}
			</h1>
			<p class="text-muted-foreground">ติดตามการเรียนและความเป็นอยู่ของบุตรหลาน</p>
		</div>
	</div>

	{#if loading}
		<div class="flex justify-center py-12">
			<Loader2 class="w-8 h-8 animate-spin text-primary" />
		</div>
	{:else if error}
		<div class="p-8 text-center text-destructive bg-destructive/10 rounded-lg">
			<p>{error}</p>
			<Button variant="outline" class="mt-4" onclick={() => window.location.reload()}
				>ลองอีกครั้ง</Button
			>
		</div>
	{:else if profile}
		<!-- Children List -->
		<div>
			<h2 class="text-xl font-semibold mb-4">บุตรหลานของคุณ</h2>

			{#if profile.children.length === 0}
				<Card class="p-8 text-center text-muted-foreground bg-muted/20 border-dashed">
					<User class="w-12 h-12 mx-auto mb-4 opacity-50" />
					<p>ไม่พบข้อมูลบุตรหลานที่เชื่อมโยงกับบัญชีของคุณ</p>
					<p class="text-sm mt-2">กรุณาติดต่อทางโรงเรียนหากข้อมูลไม่ถูกต้อง</p>
				</Card>
			{:else}
				<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
					{#each profile.children as child}
						<Card
							class="overflow-hidden hover:shadow-lg transition-all cursor-pointer group"
							onclick={() => goto(`/parent/student/${child.id}`)}
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
	{/if}
</div>
