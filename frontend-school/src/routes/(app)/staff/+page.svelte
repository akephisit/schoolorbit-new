<script lang="ts">
	import { onMount } from 'svelte';
	import { getUserMenu, type MenuGroup } from '$lib/api/menu';
	import { getStaffDashboard, type StaffDashboardOverview } from '$lib/api/staff';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { buildSidebarNavigation } from '$lib/components/layout/sidebar-navigation';
	import { Button } from '$lib/components/ui/button';
	import {
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { authStore } from '$lib/stores/auth';
	import { workStore } from '$lib/stores/work';
	import { getIconComponent } from '$lib/utils/icon-mapper';
	import {
		ArrowRight,
		Building2,
		CircleAlert,
		Clock3,
		GraduationCap,
		Inbox,
		LayoutGrid,
		RefreshCw,
		Users
	} from 'lucide-svelte';

	let stats = $state<StaffDashboardOverview | null>(null);
	let menuGroups = $state<MenuGroup[]>([]);
	let loadingStats = $state(true);
	let loadingMenu = $state(true);
	let statsError = $state('');
	let menuError = $state('');

	const numberFormatter = new Intl.NumberFormat('th-TH');
	const displayName = $derived(
		[$authStore.user?.firstName, $authStore.user?.lastName].filter(Boolean).join(' ') || 'ผู้ใช้งาน'
	);
	const roleName = $derived($authStore.user?.primaryRoleName || $authStore.user?.role || 'บุคลากร');
	const navigation = $derived(buildSidebarNavigation(menuGroups));
	const serviceWorkspaces = $derived(
		navigation
			.map((workspace) => ({
				...workspace,
				sections: workspace.sections
					.map((section) => ({
						...section,
						items: section.items.filter((item) => item.path !== '/staff')
					}))
					.filter((section) => section.items.length > 0)
			}))
			.filter((workspace) => workspace.sections.length > 0)
	);
	const accessibleServiceCount = $derived(
		navigation.reduce(
			(total, workspace) =>
				total +
				workspace.sections.reduce(
					(sectionTotal, section) => sectionTotal + section.items.length,
					0
				),
			0
		)
	);

	async function loadDashboard() {
		loadingStats = true;
		statsError = '';
		try {
			const response = await getStaffDashboard();
			if (!response.success || !response.data) {
				throw new Error(response.error || 'ไม่สามารถโหลดภาพรวมโรงเรียนได้');
			}
			stats = response.data;
		} catch (error) {
			statsError = error instanceof Error ? error.message : 'ไม่สามารถโหลดภาพรวมโรงเรียนได้';
		} finally {
			loadingStats = false;
		}
	}

	async function loadMenu() {
		loadingMenu = true;
		menuError = '';
		try {
			const response = await getUserMenu();
			menuGroups = response.groups;
		} catch (error) {
			menuError = error instanceof Error ? error.message : 'ไม่สามารถโหลดเมนูบริการได้';
		} finally {
			loadingMenu = false;
		}
	}

	onMount(() => {
		void Promise.all([loadDashboard(), loadMenu(), workStore.fetchCounts()]);
	});
</script>

<PageShell title="หน้าหลักของฉัน" description="งานที่ต้องติดตามและบริการของโรงเรียนที่คุณใช้งานได้">
	<Card
		class="gap-0 overflow-hidden border-primary/20 bg-gradient-to-br from-primary/10 via-card to-card py-0"
	>
		<CardContent class="p-5 sm:p-6">
			<div class="flex flex-col justify-between gap-5 sm:flex-row sm:items-center">
				<div class="space-y-1">
					<p class="text-sm font-medium text-primary">ยินดีต้อนรับกลับ</p>
					<h2 class="text-2xl font-semibold tracking-tight">{displayName}</h2>
					<p class="text-sm text-muted-foreground">{roleName}</p>
				</div>
				<Button href="/staff/work" class="gap-2 self-start sm:self-auto">
					<Inbox class="h-4 w-4" />
					เปิดงานของฉัน
					<ArrowRight class="h-4 w-4" />
				</Button>
			</div>
		</CardContent>
	</Card>

	<section aria-labelledby="personal-summary-title" class="space-y-3">
		<div>
			<h2 id="personal-summary-title" class="text-lg font-semibold">สรุปของฉัน</h2>
			<p class="text-sm text-muted-foreground">รายการที่ควรทราบก่อนเริ่มงานวันนี้</p>
		</div>
		<div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
			<Card class="gap-0 py-0">
				<CardContent class="flex items-center justify-between gap-4 p-4">
					<div>
						<p class="text-sm text-muted-foreground">งานที่เปิดอยู่</p>
						<p class="text-2xl font-semibold">{numberFormatter.format($workStore.counts.open)}</p>
					</div>
					<div class="rounded-lg bg-sky-500/10 p-3 text-sky-600">
						<Inbox class="h-5 w-5" />
					</div>
				</CardContent>
			</Card>
			<Card class="gap-0 py-0">
				<CardContent class="flex items-center justify-between gap-4 p-4">
					<div>
						<p class="text-sm text-muted-foreground">ใกล้ครบกำหนด</p>
						<p class="text-2xl font-semibold">
							{numberFormatter.format($workStore.counts.dueSoon)}
						</p>
					</div>
					<div class="rounded-lg bg-amber-500/10 p-3 text-amber-600">
						<Clock3 class="h-5 w-5" />
					</div>
				</CardContent>
			</Card>
			<Card class="gap-0 py-0">
				<CardContent class="flex items-center justify-between gap-4 p-4">
					<div>
						<p class="text-sm text-muted-foreground">เกินกำหนด</p>
						<p class="text-2xl font-semibold text-destructive">
							{numberFormatter.format($workStore.counts.overdue)}
						</p>
					</div>
					<div class="rounded-lg bg-destructive/10 p-3 text-destructive">
						<CircleAlert class="h-5 w-5" />
					</div>
				</CardContent>
			</Card>
			<Card class="gap-0 py-0">
				<CardContent class="flex items-center justify-between gap-4 p-4">
					<div>
						<p class="text-sm text-muted-foreground">บริการที่เข้าถึงได้</p>
						<p class="text-2xl font-semibold">{numberFormatter.format(accessibleServiceCount)}</p>
					</div>
					<div class="rounded-lg bg-violet-500/10 p-3 text-violet-600">
						<LayoutGrid class="h-5 w-5" />
					</div>
				</CardContent>
			</Card>
		</div>
	</section>

	<section aria-labelledby="services-title" class="space-y-4">
		<div>
			<h2 id="services-title" class="text-lg font-semibold">บริการของโรงเรียน</h2>
			<p class="text-sm text-muted-foreground">
				เลือกกลุ่มบริหารและฝ่าย/งาน เหมือนไปติดต่อหน่วยงานภายในโรงเรียน
			</p>
		</div>

		{#if loadingMenu}
			<PageSkeleton variant="cards" rows={4} />
		{:else if menuError}
			<PageState
				variant="error"
				title="โหลดบริการไม่สำเร็จ"
				description={menuError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadMenu}
			/>
		{:else if serviceWorkspaces.length === 0}
			<PageState
				title="ยังไม่มีบริการที่ใช้งานได้"
				description="เมนูจะปรากฏที่นี่เมื่อบัญชีได้รับสิทธิ์ในระบบที่เกี่ยวข้อง"
			/>
		{:else}
			<div class="grid items-start gap-4 xl:grid-cols-2">
				{#each serviceWorkspaces as workspace (workspace.code)}
					{@const WorkspaceIcon = getIconComponent(workspace.icon)}
					<Card>
						<CardHeader class="border-b">
							<div class="flex items-center gap-3">
								<div class="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
									<WorkspaceIcon class="h-5 w-5 text-primary" />
								</div>
								<div>
									<CardTitle>{workspace.name}</CardTitle>
									<CardDescription>
										{workspace.sections.reduce((total, section) => total + section.items.length, 0)}
										บริการ
									</CardDescription>
								</div>
							</div>
						</CardHeader>
						<CardContent class="space-y-5 p-4">
							{#each workspace.sections as section (section.id)}
								{@const SectionIcon = getIconComponent(section.icon)}
								<div class="space-y-2">
									<div class="flex items-center gap-2">
										<SectionIcon class="h-4 w-4 text-muted-foreground" />
										<h3 class="text-sm font-semibold">{section.name}</h3>
									</div>
									<div class="grid gap-2 sm:grid-cols-2">
										{#each section.items as item (item.id)}
											{@const ItemIcon = getIconComponent(item.icon)}
											<Button
												variant="outline"
												href={item.path}
												class="h-auto min-h-11 justify-start gap-2.5 whitespace-normal px-3 py-2.5 text-left"
											>
												<ItemIcon class="h-4 w-4 shrink-0 text-primary" />
												<span>{item.name}</span>
											</Button>
										{/each}
									</div>
								</div>
							{/each}
						</CardContent>
					</Card>
				{/each}
			</div>
		{/if}
	</section>

	<section aria-labelledby="school-summary-title" class="space-y-3">
		<div class="flex items-center justify-between gap-3">
			<div>
				<h2 id="school-summary-title" class="text-lg font-semibold">ภาพรวมโรงเรียน</h2>
				<p class="text-sm text-muted-foreground">ข้อมูลรวมสำหรับประกอบการทำงาน</p>
			</div>
			<Button
				variant="ghost"
				size="sm"
				class="gap-2"
				onclick={loadDashboard}
				disabled={loadingStats}
			>
				<RefreshCw class={`h-4 w-4 ${loadingStats ? 'animate-spin' : ''}`} />
				รีเฟรช
			</Button>
		</div>

		{#if loadingStats}
			<PageSkeleton variant="cards" rows={3} />
		{:else if statsError}
			<PageState
				variant="error"
				title="โหลดภาพรวมโรงเรียนไม่สำเร็จ"
				description={statsError}
				actionLabel="ลองอีกครั้ง"
				onaction={loadDashboard}
			/>
		{:else if stats}
			<div class="grid gap-3 md:grid-cols-3">
				<Card class="gap-0 py-0">
					<CardContent class="flex items-center justify-between gap-4 p-4">
						<div>
							<p class="text-sm text-muted-foreground">บุคลากรทั้งหมด</p>
							<p class="text-2xl font-semibold">{numberFormatter.format(stats.totalStaff)}</p>
						</div>
						<Users class="h-5 w-5 text-sky-600" />
					</CardContent>
				</Card>
				<Card class="gap-0 py-0">
					<CardContent class="flex items-center justify-between gap-4 p-4">
						<div>
							<p class="text-sm text-muted-foreground">นักเรียนทั้งหมด</p>
							<p class="text-2xl font-semibold">{numberFormatter.format(stats.totalStudents)}</p>
						</div>
						<GraduationCap class="h-5 w-5 text-emerald-600" />
					</CardContent>
				</Card>
				<Card class="gap-0 py-0">
					<CardContent class="flex items-center justify-between gap-4 p-4">
						<div>
							<p class="text-sm text-muted-foreground">ห้องเรียนที่เปิด</p>
							<p class="text-2xl font-semibold">{numberFormatter.format(stats.activeClassrooms)}</p>
						</div>
						<Building2 class="h-5 w-5 text-amber-600" />
					</CardContent>
				</Card>
			</div>
		{/if}
	</section>
</PageShell>
