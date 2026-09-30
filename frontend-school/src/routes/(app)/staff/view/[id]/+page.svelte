<script lang="ts">
	import { untrack } from 'svelte';
	import { get } from 'svelte/store';
	import { authStore } from '$lib/stores/auth';
	import { can } from '$lib/stores/permissions';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { LatestRequest } from '$lib/async/latest-request';
	import { captureRouteLoad } from '$lib/navigation/route-load';
	import { requireApiData } from '$lib/api/client';
	import type { PageProps } from './$types';
	import { resolve } from '$app/paths';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import {
		Card,
		CardContent,
		CardHeader,
		CardTitle,
		CardDescription
	} from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import * as Dialog from '$lib/components/ui/dialog';
	import {
		Building2,
		Briefcase,
		Mail,
		Phone,
		Calendar,
		Award,
		User,
		FileText
	} from '@lucide/svelte';
	import { getPublicStaffProfile } from '$lib/api/staff';
	import { getAchievements } from '$lib/api/achievement';
	import type { PublicStaffProfileResponse } from '$lib/api/staff';
	import type { Achievement } from '$lib/types/achievement';
	import PrivateFileImage from '$lib/components/files/PrivateFileImage.svelte';

	let { data }: PageProps = $props();
	const staffId = $derived(data.staffId);
	const liveAchievementsReadable = $derived(
		$can.has(PERMISSIONS.ACHIEVEMENT_READ_ALL) ||
			($authStore.user?.id === staffId && $can.has(PERMISSIONS.ACHIEVEMENT_READ_OWN))
	);
	let staff = $state<PublicStaffProfileResponse | null>(null);
	let achievements = $state<Achievement[]>([]);
	let loading = $state(true);
	let loadingAchievements = $state(true);
	let error = $state('');
	let achievementsError = $state('');
	let achievementsLoaded = $state(false);
	let achievementsReadable = $state(false);
	let renderedId = '';
	const profileRequest = new LatestRequest();
	const achievementsRequest = new LatestRequest();

	// File Preview State
	let showFileDialog = $state(false);
	let viewingFileId = $state('');
	let viewingResourceId = $state('');

	function viewFile(fileId: string, resourceId: string) {
		viewingFileId = fileId;
		viewingResourceId = resourceId;
		showFileDialog = true;
	}

	$effect.pre(() => {
		const source = data;
		untrack(() => {
			if (renderedId !== source.staffId) {
				renderedId = source.staffId;
				staff = null;
				achievements = [];
				achievementsLoaded = false;
				achievementsReadable = false;
				showFileDialog = false;
				viewingFileId = '';
				viewingResourceId = '';
			}
			const profileTicket = profileRequest.begin();
			const achievementsTicket = achievementsRequest.begin();
			loading = true;
			error = '';
			loadingAchievements = true;
			achievementsError = '';
			void source.profile.then((result) => {
				if (!profileRequest.isCurrent(profileTicket.revision)) return;
				loading = false;
				if (result.ok) staff = result.data;
				else error = result.error;
			});
			void source.achievements.then((result) => {
				if (!achievementsRequest.isCurrent(achievementsTicket.revision)) return;
				loadingAchievements = false;
				if (result.ok) {
					achievements = result.data.items;
					achievementsReadable = result.data.readable;
					achievementsLoaded = true;
				} else achievementsError = result.error;
			});
		});
		return () => {
			profileRequest.abort();
			achievementsRequest.abort();
		};
	});
	$effect.pre(() => {
		if (liveAchievementsReadable) return;
		untrack(() => {
			achievementsRequest.abort();
			achievements = [];
			achievementsReadable = false;
			achievementsLoaded = false;
			loadingAchievements = false;
			achievementsError = '';
			showFileDialog = false;
			viewingFileId = '';
			viewingResourceId = '';
		});
	});

	async function loadStaffProfile() {
		const ticket = profileRequest.begin();
		loading = true;
		error = '';
		const result = await captureRouteLoad(
			getPublicStaffProfile(staffId, { signal: ticket.signal }).then((response) =>
				requireApiData(response, 'โหลดบุคลากรไม่สำเร็จ')
			),
			'โหลดบุคลากรไม่สำเร็จ'
		);
		if (!profileRequest.isCurrent(ticket.revision)) return;
		loading = false;
		if (result.ok) staff = result.data;
		else error = result.error;
	}
	async function loadAchievements() {
		if (
			!get(can).has(PERMISSIONS.ACHIEVEMENT_READ_ALL) &&
			!(get(authStore).user?.id === staffId && get(can).has(PERMISSIONS.ACHIEVEMENT_READ_OWN))
		)
			return;
		const ticket = achievementsRequest.begin();
		loadingAchievements = true;
		achievementsError = '';
		const result = await captureRouteLoad(
			getAchievements({ user_id: staffId }, { signal: ticket.signal }).then((response) =>
				requireApiData(response, 'โหลดผลงานไม่สำเร็จ')
			),
			'โหลดผลงานไม่สำเร็จ'
		);
		if (!achievementsRequest.isCurrent(ticket.revision)) return;
		loadingAchievements = false;
		if (result.ok) {
			achievements = result.data;
			achievementsLoaded = true;
			achievementsReadable = true;
		} else achievementsError = result.error;
	}

	function formatDate(dateStr: string) {
		if (!dateStr) return '-';
		return new Date(dateStr).toLocaleDateString('th-TH', {
			year: 'numeric',
			month: 'long',
			day: 'numeric'
		});
	}
</script>

<PageShell
	title="ผลงานบุคลากร"
	description={staff
		? `${staff.title ?? ''}${staff.first_name} ${staff.last_name}`
		: 'ข้อมูลบุคลากรและผลงาน'}
	backHref="/staff/achievements"
>
	<div class="grid min-w-0 gap-6 xl:grid-cols-[minmax(0,1fr)_minmax(0,2fr)]">
		<div data-testid="staff-public-profile" aria-busy={loading}>
			{#if loading && !staff}
				<div role="status" aria-label="กำลังโหลดบุคลากร"><PageSkeleton variant="detail" /></div>
			{:else if error && !staff}
				<PageState
					variant="error"
					title="โหลดข้อมูลบุคลากรไม่สำเร็จ"
					description={error}
					actionLabel="ลองใหม่"
					onaction={loadStaffProfile}
				/>
			{:else if staff}
				{#if loading}<p role="status">กำลังอัปเดตบุคลากร...</p>{/if}
				{#if error}<PageState
						title="อัปเดตบุคลากรไม่สำเร็จ"
						description={error}
						actionLabel="ลองใหม่"
						onaction={loadStaffProfile}
					/>{/if}
				<!-- Header Profile Card -->
				<Card
					class="gap-0 overflow-hidden border-none bg-gradient-to-br from-primary/5 via-card to-card py-0 shadow-lg"
				>
					<CardContent class="p-8">
						<div class="flex flex-col md:flex-row items-center gap-8">
							<!-- Avatar -->
							<div class="relative group">
								<div
									class="absolute -inset-1 rounded-full bg-gradient-to-r from-primary to-primary/50 opacity-30 blur group-hover:opacity-60 transition-opacity"
								></div>
								<div
									class="w-32 h-32 overflow-hidden rounded-full border-4 border-background relative shadow-xl bg-muted flex items-center justify-center"
								>
									{#if staff.profile_image_file_id}
										<PrivateFileImage
											fileId={staff.profile_image_file_id}
											resourceId={staff.id}
											alt={staff.first_name}
											class="w-full h-full object-cover"
										/>
									{:else}
										<User class="w-14 h-14 text-muted-foreground/50" />
									{/if}
								</div>
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
										{#each staff.roles as role (role.id || role.name)}
											<Badge
												variant="secondary"
												class="bg-primary/10 text-primary hover:bg-primary/20 border-primary/20"
											>
												<Briefcase class="w-3 h-3 mr-1" />
												{role.name}
											</Badge>
										{/each}
									{/if}

									{#if staff.organization_units && staff.organization_units.length > 0}
										{#each staff.organization_units as dept (dept.id || dept.name)}
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

				<div class="space-y-6">
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
				</div>
			{:else}
				<PageState
					title="ไม่พบข้อมูลบุคลากร"
					description="ข้อมูลบุคลากรนี้อาจถูกลบหรือคุณอาจไม่มีสิทธิ์เข้าถึง"
					actionLabel="กลับหน้าผลงาน"
					href={resolve('/staff/achievements')}
				/>
			{/if}
		</div>
		<div data-testid="staff-public-achievements" aria-busy={loadingAchievements}>
			<Card class="flex-1 h-full">
				<CardHeader>
					<CardTitle class="text-xl flex items-center gap-2">
						<Award class="w-6 h-6 text-yellow-500" />
						ผลงานและรางวัลที่ได้รับ
					</CardTitle>
					<CardDescription>รายการความภาคภูมิใจและเกียรติบัตรทั้งหมด</CardDescription>
				</CardHeader>
				<CardContent>
					{#if loadingAchievements && achievementsLoaded}<p role="status">
							กำลังอัปเดตผลงาน...
						</p>{/if}
					{#if achievementsError && achievementsLoaded}<PageState
							title="อัปเดตผลงานไม่สำเร็จ"
							description={achievementsError}
							actionLabel="ลองใหม่"
							onaction={loadAchievements}
						/>{/if}
					{#if !liveAchievementsReadable}<PageState title="ไม่มีสิทธิ์ดูผลงานของบุคลากรนี้" />
					{:else if loadingAchievements && !achievementsLoaded}
						<div role="status" aria-label="กำลังโหลดผลงาน">
							<PageSkeleton variant="cards" rows={2} />
						</div>
					{:else if achievementsError && !achievementsLoaded}<PageState
							title="โหลดผลงานไม่สำเร็จ"
							description={achievementsError}
							actionLabel="ลองใหม่"
							onaction={loadAchievements}
						/>
					{:else if !achievementsReadable}<PageState title="ไม่มีสิทธิ์ดูผลงานของบุคลากรนี้" />
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
							{#each achievements as item (item.id || item.title)}
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

										{#if item.image_file_id}
											<button
												onclick={() => viewFile(item.image_file_id!, item.id)}
												class="inline-flex items-center gap-1.5 text-xs font-medium text-primary hover:text-primary/80 transition-colors bg-primary/5 px-2.5 py-1.5 rounded-md cursor-pointer border-0"
											>
												<FileText class="w-3.5 h-3.5" />
												ดูเอกสาร/เกียรติบัตร
											</button>
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
	<!-- File Preview Dialog -->
	<Dialog.Root bind:open={showFileDialog}>
		<Dialog.Content
			class="max-w-[95vw] md:max-w-7xl max-h-[95vh] overflow-hidden flex flex-col p-0 gap-0"
		>
			<div
				class="relative flex-1 bg-muted/30 min-h-[200px] flex items-center justify-center overflow-auto p-4"
			>
				{#if viewingFileId}
					<PrivateFileImage
						fileId={viewingFileId}
						resourceId={viewingResourceId}
						alt="Preview"
						class="max-w-full max-h-[80vh] object-contain shadow-sm rounded-sm"
					/>
				{:else}
					<div class="text-center p-8">
						<p class="mb-4 text-muted-foreground">ไม่สามารถแสดงตัวอย่างไฟล์ประเภทนี้ได้</p>
					</div>
				{/if}
			</div>
			<div class="p-4 border-t flex justify-end bg-background">
				<Button variant="outline" onclick={() => (showFileDialog = false)}>ปิด</Button>
			</div>
		</Dialog.Content>
	</Dialog.Root>
</PageShell>
