<script lang="ts">
	import { untrack } from 'svelte';
	import { page } from '$app/state';
	import type { PageProps } from './$types';
	import { captureRouteLoad } from '#lib/navigation/route-load.js';
	import { requireApiData } from '#lib/api/client.js';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { menuPreloadPolicy } from '#lib/navigation/menu-preload.js';
	import { getAppMenuRegion } from '#lib/navigation/app-menu.svelte.js';
	import {
		academicContextualMenuPath,
		getAcademicContextOptionsSource,
		readAcademicContextFromUrl
	} from '#lib/academic-context/route-context.js';
	import { getAcademicContextStore } from '#lib/academic-context/store.js';
	import { getStaffDashboard, type StaffDashboardOverview } from '#lib/api/staff.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { buildSidebarNavigation } from '#lib/components/layout/sidebar-navigation.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Card, CardContent, CardHeader, CardTitle } from '#lib/components/ui/card/index.js';
	import { authStore } from '#lib/stores/auth.js';
	import { workStore } from '#lib/stores/work.js';
	import { getIconComponent } from '#lib/utils/icon-mapper.js';
	import {
		ArrowRight,
		Building2,
		ChevronRight,
		CircleAlert,
		Clock3,
		GraduationCap,
		Inbox,
		Users
	} from '@lucide/svelte';

	let { data }: PageProps = $props();
	const appMenu = getAppMenuRegion();
	const academicContext = getAcademicContextStore();
	const academicYearId = $derived(data.academicYearId ?? '');
	const menuContext = $derived(readAcademicContextFromUrl(new URL(page.url.href)));
	let stats = $state<StaffDashboardOverview | null>(null);
	const menuGroups = $derived(appMenu.groups);
	let loadingStats = $state(true);
	const loadingMenu = $derived(appMenu.loading);
	let statsError = $state('');
	const menuError = $derived(appMenu.error);
	const statsRequest = new LatestRequest();
	let statsYear = '';
	let showAllServices = $state(false);
	const servicePreviewLimit = 4;

	const numberFormatter = new Intl.NumberFormat('th-TH');
	const displayName = $derived($authStore.user?.displayName || 'ผู้ใช้งาน');
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
		serviceWorkspaces.reduce(
			(total, workspace) =>
				total +
				workspace.sections.reduce(
					(sectionTotal, section) => sectionTotal + section.items.length,
					0
				),
			0
		)
	);

	const visibleServiceWorkspaces = $derived.by(() => {
		if (showAllServices) return serviceWorkspaces;
		let remaining = servicePreviewLimit;
		return serviceWorkspaces
			.map((workspace) => ({
				...workspace,
				sections: workspace.sections
					.map((section) => {
						const items = section.items.slice(0, remaining);
						remaining -= items.length;
						return { ...section, items };
					})
					.filter((section) => section.items.length > 0)
			}))
			.filter((workspace) => workspace.sections.length > 0);
	});

	$effect.pre(() => {
		const source = data;
		untrack(() => {
			if (statsYear !== source.academicYearId) {
				statsYear = source.academicYearId ?? '';
				stats = null;
			}
			const ticket = statsRequest.begin();
			loadingStats = source.overview !== null;
			statsError = '';
			if (source.overview)
				void source.overview.then((result) => {
					if (!statsRequest.isCurrent(ticket.revision)) return;
					loadingStats = false;
					if (result.ok) stats = result.data;
					else statsError = result.error;
				});
		});
		return () => statsRequest.abort();
	});

	async function loadDashboard(yearId: string) {
		if (!yearId) return;
		const ticket = statsRequest.begin();
		loadingStats = true;
		statsError = '';
		const result = await captureRouteLoad(
			getStaffDashboard(yearId, { signal: ticket.signal }).then((response) =>
				requireApiData(response, 'โหลดภาพรวมโรงเรียนไม่สำเร็จ')
			),
			'โหลดภาพรวมโรงเรียนไม่สำเร็จ'
		);
		if (!statsRequest.isCurrent(ticket.revision)) return;
		loadingStats = false;
		if (result.ok) stats = result.data;
		else statsError = result.error;
	}

	function serviceHref(path: string): string {
		const source = getAcademicContextOptionsSource(
			`/(app)${new URL(path, page.url.href).pathname}`
		);
		return academicContextualMenuPath(path, menuContext, $academicContext.optionsBySource[source]);
	}
</script>

<PageShell title="หน้าหลักของฉัน">
	<Card class="gap-0 border-primary/20 bg-primary/5 py-0 shadow-none">
		<CardContent class="flex flex-wrap items-center justify-between gap-3 p-4">
			<div class="min-w-0 space-y-1">
				<h2 class="text-lg font-semibold break-words">สวัสดี {displayName}</h2>
				<p class="text-sm text-muted-foreground">{roleName}</p>
			</div>
			<Button href="/staff/work" class="gap-2">
				<Inbox class="size-4" aria-hidden="true" />
				เปิดงานของฉัน
				<ArrowRight class="size-4" aria-hidden="true" />
			</Button>
		</CardContent>
	</Card>
	<section
		data-testid="staff-work-counts"
		aria-busy={$workStore.loadingCounts}
		aria-labelledby="personal-summary-title"
		class="space-y-3"
	>
		<h2 id="personal-summary-title" class="text-lg font-semibold">สรุปงานของฉัน</h2>
		{#if $workStore.countsError}<PageState
				title="โหลดจำนวนงานไม่สำเร็จ"
				description={$workStore.countsError}
				actionLabel="ลองใหม่"
				onaction={() => void workStore.fetchCounts()}
			/>{/if}
		{#if $workStore.loadingCounts && !$workStore.loadedCounts}<p
				role="status"
				class="text-sm text-muted-foreground"
			>
				กำลังโหลดจำนวนงาน...
			</p>{/if}
		<Card class="gap-0 py-0 shadow-none">
			<CardContent class="p-0">
				<dl class="grid grid-cols-3 divide-x">
					<div class="min-w-0 space-y-2 p-3 sm:p-4">
						<dt class="flex flex-wrap items-center gap-1.5 text-sm text-muted-foreground">
							<Inbox class="size-4 shrink-0 text-primary" aria-hidden="true" />งานที่เปิดอยู่
						</dt>
						<dd class="text-2xl font-semibold tabular-nums">
							{$workStore.loadedCounts ? numberFormatter.format($workStore.counts.open) : '—'}
						</dd>
					</div>
					<div class="min-w-0 space-y-2 p-3 sm:p-4">
						<dt class="flex flex-wrap items-center gap-1.5 text-sm text-muted-foreground">
							<Clock3 class="size-4 shrink-0 text-warning" aria-hidden="true" />ใกล้ครบกำหนด
						</dt>
						<dd class="text-2xl font-semibold text-warning tabular-nums">
							{$workStore.loadedCounts ? numberFormatter.format($workStore.counts.dueSoon) : '—'}
						</dd>
					</div>
					<div class="min-w-0 space-y-2 p-3 sm:p-4">
						<dt class="flex flex-wrap items-center gap-1.5 text-sm text-muted-foreground">
							<CircleAlert class="size-4 shrink-0 text-destructive" aria-hidden="true" />เกินกำหนด
						</dt>
						<dd class="text-2xl font-semibold text-destructive tabular-nums">
							{$workStore.loadedCounts ? numberFormatter.format($workStore.counts.overdue) : '—'}
						</dd>
					</div>
				</dl>
			</CardContent>
		</Card>
	</section>
	<div class="grid items-start gap-6 xl:grid-cols-[minmax(0,1fr)_16rem]">
		<section
			data-testid="staff-services"
			aria-busy={loadingMenu}
			aria-labelledby="services-title"
			class="min-w-0 space-y-3"
		>
			{#if menuError && appMenu.loaded}<PageState
					title="อัปเดตบริการไม่สำเร็จ"
					description={menuError}
					actionLabel="ลองอีกครั้ง"
					onaction={appMenu.retry}
				/>{/if}

			<div class="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
				<h2 id="services-title" class="text-lg font-semibold">บริการของโรงเรียน</h2>
				{#if appMenu.loaded}
					<p class="text-sm text-muted-foreground">
						{numberFormatter.format(accessibleServiceCount)} บริการ
					</p>
				{/if}
			</div>

			{#if loadingMenu && !appMenu.loaded}
				<div role="status" aria-label="กำลังโหลดบริการ">
					<PageSkeleton variant="cards" rows={4} />
				</div>
			{:else if menuError && !appMenu.loaded}
				<PageState
					variant="error"
					title="โหลดบริการไม่สำเร็จ"
					description={menuError}
					actionLabel="ลองอีกครั้ง"
					onaction={appMenu.retry}
				/>
			{:else if serviceWorkspaces.length === 0}
				<PageState
					title="ยังไม่มีบริการที่ใช้งานได้"
					description="เมนูจะปรากฏที่นี่เมื่อบัญชีได้รับสิทธิ์ในระบบที่เกี่ยวข้อง"
				/>
			{:else}
				{#if accessibleServiceCount > servicePreviewLimit}
					<Button
						variant="outline"
						aria-expanded={showAllServices}
						aria-controls="staff-service-catalogue"
						onclick={() => (showAllServices = !showAllServices)}
					>
						{showAllServices ? 'แสดงบริการน้อยลง' : 'ดูบริการทั้งหมด'}
					</Button>
				{/if}
				<div
					id="staff-service-catalogue"
					class={visibleServiceWorkspaces.length > 1
						? 'columns-1 gap-4 xl:columns-2'
						: 'columns-1 gap-4'}
					data-testid="staff-service-workspaces"
				>
					{#each visibleServiceWorkspaces as workspace (workspace.code)}
						{@const WorkspaceIcon = getIconComponent(workspace.icon)}
						<Card
							class="mb-4 w-full break-inside-avoid gap-0 py-0 shadow-none"
							data-workspace={workspace.code}
						>
							<CardHeader class="border-b px-3 py-3 sm:px-4">
								<div class="flex items-center gap-3">
									<WorkspaceIcon class="size-5 shrink-0 text-primary" aria-hidden="true" />
									<CardTitle class="text-base leading-snug">{workspace.name}</CardTitle>
								</div>
							</CardHeader>
							<CardContent class="space-y-4 p-3 sm:p-4">
								{#each workspace.sections as section (section.id)}
									{@const SectionIcon = getIconComponent(section.icon)}
									<div class="space-y-2">
										<div class="flex items-center gap-2">
											<SectionIcon
												class="size-4 shrink-0 text-muted-foreground"
												aria-hidden="true"
											/>
											<h3 class="text-sm font-semibold">{section.name}</h3>
										</div>
										<ul class="grid items-start gap-2 min-[360px]:grid-cols-2">
											{#each section.items as item (item.id)}
												{@const ItemIcon = getIconComponent(item.icon)}
												<li class="min-w-0">
													<Button
														variant="outline"
														href={serviceHref(item.path)}
														data-sveltekit-preload-data={menuPreloadPolicy(item)}
														class="h-auto min-h-24 w-full flex-col items-start justify-start gap-2 rounded-xl px-3 py-3 text-start whitespace-normal shadow-none"
													>
														<span class="flex w-full items-center justify-between gap-2">
															<span
																class="inline-flex size-8 shrink-0 items-center justify-center rounded-lg bg-muted text-foreground"
																><ItemIcon class="size-4" aria-hidden="true" /></span
															>
															<ChevronRight
																class="size-4 shrink-0 text-muted-foreground"
																aria-hidden="true"
															/>
														</span>
														<span class="break-words leading-snug">{item.name}</span>
													</Button>
												</li>
											{/each}
										</ul>
									</div>
								{/each}
							</CardContent>
						</Card>
					{/each}
				</div>
			{/if}
		</section>

		<section
			data-testid="staff-overview"
			aria-busy={loadingStats}
			aria-labelledby="school-summary-title"
			class="space-y-3"
		>
			{#if statsError && stats}<PageState
					title="อัปเดตภาพรวมไม่สำเร็จ"
					description={statsError}
					actionLabel="ลองอีกครั้ง"
					onaction={() => void loadDashboard(academicYearId)}
				/>{/if}
			<h2 id="school-summary-title" class="text-lg font-semibold">ภาพรวมโรงเรียน</h2>

			{#if !academicYearId}
				<PageState
					variant="empty"
					title="เลือกปีการศึกษาก่อน"
					description="ใช้ตัวเลือกปีการศึกษาบนแถบด้านบน"
				/>
			{:else if loadingStats && !stats}
				<div role="status" aria-label="กำลังโหลดภาพรวม">
					<PageSkeleton variant="cards" rows={3} />
				</div>
			{:else if statsError && !stats}
				<PageState
					variant="error"
					title="โหลดภาพรวมโรงเรียนไม่สำเร็จ"
					description={statsError}
					actionLabel="ลองอีกครั้ง"
					onaction={() => loadDashboard(academicYearId)}
				/>
			{:else if stats}
				<Card class="gap-0 py-0 shadow-none">
					<CardContent class="px-4 py-1">
						<dl class="divide-y">
							<div class="flex items-center justify-between gap-3 py-3">
								<dt class="flex items-center gap-2 text-sm text-muted-foreground">
									<Users class="size-4 shrink-0" aria-hidden="true" />บุคลากรทั้งหมด
								</dt>
								<dd class="text-lg font-semibold tabular-nums">
									{numberFormatter.format(stats.totalStaff)}
								</dd>
							</div>
							<div class="flex items-center justify-between gap-3 py-3">
								<dt class="flex items-center gap-2 text-sm text-muted-foreground">
									<GraduationCap class="size-4 shrink-0" aria-hidden="true" />นักเรียนทั้งหมด
								</dt>
								<dd class="text-lg font-semibold tabular-nums">
									{numberFormatter.format(stats.totalStudents)}
								</dd>
							</div>
							<div class="flex items-center justify-between gap-3 py-3">
								<dt class="flex items-center gap-2 text-sm text-muted-foreground">
									<Building2 class="size-4 shrink-0" aria-hidden="true" />ห้องเรียนที่เปิด
								</dt>
								<dd class="text-lg font-semibold tabular-nums">
									{numberFormatter.format(stats.activeHomerooms)}
								</dd>
							</div>
						</dl>
					</CardContent>
				</Card>
			{/if}
		</section>
	</div>
</PageShell>
