<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getRound,
		listApplications,
		type AdmissionRound,
		type ApplicationListItem,
		type ReportConfig,
		applicationStatusLabel
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Select from '$lib/components/ui/select';
	import { LoaderCircle, ArrowLeft, Settings, ChevronDown } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	import type { PageProps } from './$types';
	let { data, params }: PageProps = $props();

	let id = $derived(params.id);
	let round = $state<AdmissionRound | null>(null);
	let applications: ApplicationListItem[] = $state([]);
	let loading = $state(true);
	let statusFilter = $state('all');

	// ---- Types ----
	interface SchoolCount { name: string; count: number }
	interface ZoneStatsDetail {
		inZone:  { total: number; schools: SchoolCount[] }
		outZone: { total: number; schools: SchoolCount[] }
		noInfo:  { total: number }
		total:   number
	}
	interface InstitutionStatsDetail {
		fromOwn:   { total: number }
		fromOther: { total: number; schools: SchoolCount[] }
		noInfo:    { total: number }
		total:     number
	}
	interface DayGroup { date: string; apps: ApplicationListItem[] }

	// ---- Helpers ----
	function countBySchool(subset: ApplicationListItem[]): SchoolCount[] {
		const map = new Map<string, number>();
		for (const a of subset) {
			const name = a.previousSchool!;
			map.set(name, (map.get(name) ?? 0) + 1);
		}
		return Array.from(map.entries())
			.map(([name, count]) => ({ name, count }))
			.sort((a, b) => b.count - a.count);
	}

	function computeZoneDetail(apps: ApplicationListItem[], zoneSchools: string[]): ZoneStatsDetail {
		const inZoneApps  = apps.filter(a => a.previousSchool && zoneSchools.includes(a.previousSchool));
		const noInfoApps  = apps.filter(a => !a.previousSchool);
		const outZoneApps = apps.filter(a => a.previousSchool && !zoneSchools.includes(a.previousSchool));
		return {
			inZone:  { total: inZoneApps.length,  schools: countBySchool(inZoneApps) },
			outZone: { total: outZoneApps.length, schools: countBySchool(outZoneApps) },
			noInfo:  { total: noInfoApps.length },
			total:   apps.length
		};
	}

	function computeInstitutionDetail(apps: ApplicationListItem[], ownSchool: string): InstitutionStatsDetail {
		const fromOwnApps   = apps.filter(a => a.previousSchool === ownSchool);
		const noInfoApps    = apps.filter(a => !a.previousSchool);
		const fromOtherApps = apps.filter(a => a.previousSchool && a.previousSchool !== ownSchool);
		return {
			fromOwn:   { total: fromOwnApps.length },
			fromOther: { total: fromOtherApps.length, schools: countBySchool(fromOtherApps) },
			noInfo:    { total: noInfoApps.length },
			total:     apps.length
		};
	}

	function pct(n: number, total: number) {
		if (total === 0) return '0.0%';
		return ((n / total) * 100).toFixed(1) + '%';
	}

	function formatThaiDate(dateStr: string): string {
		if (dateStr === 'unknown') return 'ไม่ระบุวันที่';
		return new Date(dateStr + 'T00:00:00').toLocaleDateString('th-TH', {
			year: 'numeric', month: 'long', day: 'numeric'
		});
	}

	// ---- Expand state ----
	let expanded = $state(new Map<string, boolean>());

	function toggleExpand(key: string) {
		const next = new Map(expanded);
		next.set(key, !next.get(key));
		expanded = next;
	}
	function isExpanded(key: string): boolean {
		return expanded.get(key) ?? false;
	}

	// ---- Data loading ----
	async function load() {
		if (!id) return;
		loading = true;
		try {
			const [r, apps] = await Promise.all([getRound(id), listApplications(id, {})]);
			round = r;
			applications = apps;
		} catch (e) {
			toast.error(e instanceof Error ? e.message : 'โหลดข้อมูลไม่สำเร็จ');
		} finally {
			loading = false;
		}
	}

	let reportConfig = $derived(round ? (round.reportConfig ?? null) : null);

	let filteredApps = $derived(
		applications.filter(a => statusFilter === 'all' || a.status === statusFilter)
	);

	let dayGroups = $derived((): DayGroup[] => {
		const map = new Map<string, ApplicationListItem[]>();
		for (const a of filteredApps) {
			const date = a.createdAt?.slice(0, 10) ?? 'unknown';
			if (!map.has(date)) map.set(date, []);
			map.get(date)!.push(a);
		}
		return Array.from(map.entries())
			.sort(([a], [b]) => b.localeCompare(a))
			.map(([date, apps]) => ({ date, apps }));
	});

	const reportModeLabel: Record<string, string> = {
		zone: 'เขตพื้นที่บริการ',
		institution: 'สถานศึกษาเดิม',
		both: 'ทั้งสองประเภท'
	};

	const allStatuses = ['submitted', 'verified', 'scored', 'rejected', 'accepted', 'enrolled', 'withdrawn'];

	onMount(load);
</script>

<svelte:head>
	<title>{data.title} - SchoolOrbit</title>
</svelte:head>

{#snippet statsBlock(scopeKey: string, apps: ApplicationListItem[])}
	{@const zoneSchools = reportConfig?.zone?.schools ?? []}
	{@const ownSchool   = reportConfig?.institution?.ownSchool ?? ''}

	{#if apps.length === 0}
		<p class="text-sm text-muted-foreground py-4 text-center">ไม่มีข้อมูลในช่วงนี้</p>
	{:else}
		<div class="space-y-4">
			{#if reportConfig?.reportMode === 'zone' || reportConfig?.reportMode === 'both'}
				{@const z = computeZoneDetail(apps, zoneSchools)}
				<Card.Root>
					<Card.Header class="pb-2">
						<Card.Title class="text-base">เขตพื้นที่บริการ</Card.Title>
						{#if scopeKey === 'overall'}
							<Card.Description>
								โรงเรียนในเขต: {reportConfig?.zone?.schools?.join(', ') || '(ยังไม่ได้กำหนด)'}
							</Card.Description>
						{/if}
					</Card.Header>
					<Card.Content>
						<div class="overflow-x-auto">
							<table class="w-full text-sm min-w-[320px]">
								<thead>
									<tr class="border-b">
										<th class="text-left py-2 font-medium text-muted-foreground">ประเภท</th>
										<th class="text-right py-2 font-medium text-muted-foreground w-16">จำนวน</th>
										<th class="text-right py-2 font-medium text-muted-foreground w-16">%</th>
										<th class="py-2 w-24 hidden sm:table-cell"></th>
									</tr>
								</thead>
								<tbody>
									<!-- ในเขตพื้นที่ -->
									<tr
										class="border-b {z.inZone.schools.length > 0 ? 'cursor-pointer hover:bg-muted/40' : ''}"
										onclick={() => z.inZone.schools.length > 0 && toggleExpand(`${scopeKey}-inZone`)}
									>
										<td class="py-2">
											<span class="flex items-center gap-1.5">
												ในเขตพื้นที่
												{#if z.inZone.schools.length > 0}
													<ChevronDown class="w-3.5 h-3.5 text-muted-foreground transition-transform duration-200 {isExpanded(`${scopeKey}-inZone`) ? 'rotate-180' : ''}" />
												{/if}
											</span>
										</td>
										<td class="text-right py-2 tabular-nums">{z.inZone.total}</td>
										<td class="text-right py-2 tabular-nums">{pct(z.inZone.total, z.total)}</td>
										<td class="py-2 pl-3 hidden sm:table-cell">
											<div class="h-2 rounded-full bg-muted overflow-hidden">
												<div class="h-full bg-green-500 rounded-full" style="width: {z.total ? (z.inZone.total/z.total*100) : 0}%"></div>
											</div>
										</td>
									</tr>
									{#if isExpanded(`${scopeKey}-inZone`) && z.inZone.schools.length > 0}
										<tr>
											<td colspan="4" class="pb-3 pt-0 pl-5">
												<div class="space-y-0.5 text-xs">
													{#each z.inZone.schools as school}
														<div class="flex items-center justify-between pr-2 py-0.5 text-muted-foreground">
															<span class="flex items-center gap-1.5"><span class="opacity-40">└</span>{school.name}</span>
															<span class="tabular-nums shrink-0 ml-4">{school.count} <span class="opacity-60">({pct(school.count, z.inZone.total)})</span></span>
														</div>
													{/each}
												</div>
											</td>
										</tr>
									{/if}

									<!-- นอกเขตพื้นที่ -->
									<tr
										class="border-b {z.outZone.schools.length > 0 ? 'cursor-pointer hover:bg-muted/40' : ''}"
										onclick={() => z.outZone.schools.length > 0 && toggleExpand(`${scopeKey}-outZone`)}
									>
										<td class="py-2">
											<span class="flex items-center gap-1.5">
												นอกเขตพื้นที่
												{#if z.outZone.schools.length > 0}
													<ChevronDown class="w-3.5 h-3.5 text-muted-foreground transition-transform duration-200 {isExpanded(`${scopeKey}-outZone`) ? 'rotate-180' : ''}" />
												{/if}
											</span>
										</td>
										<td class="text-right py-2 tabular-nums">{z.outZone.total}</td>
										<td class="text-right py-2 tabular-nums">{pct(z.outZone.total, z.total)}</td>
										<td class="py-2 pl-3 hidden sm:table-cell">
											<div class="h-2 rounded-full bg-muted overflow-hidden">
												<div class="h-full bg-red-400 rounded-full" style="width: {z.total ? (z.outZone.total/z.total*100) : 0}%"></div>
											</div>
										</td>
									</tr>
									{#if isExpanded(`${scopeKey}-outZone`) && z.outZone.schools.length > 0}
										<tr>
											<td colspan="4" class="pb-3 pt-0 pl-5">
												<div class="space-y-0.5 text-xs">
													{#each z.outZone.schools as school}
														<div class="flex items-center justify-between pr-2 py-0.5 text-muted-foreground">
															<span class="flex items-center gap-1.5"><span class="opacity-40">└</span>{school.name}</span>
															<span class="tabular-nums shrink-0 ml-4">{school.count} <span class="opacity-60">({pct(school.count, z.outZone.total)})</span></span>
														</div>
													{/each}
												</div>
											</td>
										</tr>
									{/if}

									<!-- ไม่ระบุ -->
									<tr class="border-b">
										<td class="py-2 text-muted-foreground">ไม่ระบุ</td>
										<td class="text-right py-2 tabular-nums">{z.noInfo.total}</td>
										<td class="text-right py-2 tabular-nums">{pct(z.noInfo.total, z.total)}</td>
										<td class="py-2 pl-3 hidden sm:table-cell">
											<div class="h-2 rounded-full bg-muted overflow-hidden">
												<div class="h-full bg-gray-400 rounded-full" style="width: {z.total ? (z.noInfo.total/z.total*100) : 0}%"></div>
											</div>
										</td>
									</tr>
									<tr class="font-semibold">
										<td class="py-2">รวม</td>
										<td class="text-right py-2 tabular-nums">{z.total}</td>
										<td class="text-right py-2 tabular-nums">100%</td>
										<td class="hidden sm:table-cell"></td>
									</tr>
								</tbody>
							</table>
						</div>
					</Card.Content>
				</Card.Root>
			{/if}

			{#if reportConfig?.reportMode === 'institution' || reportConfig?.reportMode === 'both'}
				{@const inst = computeInstitutionDetail(apps, ownSchool)}
				<Card.Root>
					<Card.Header class="pb-2">
						<Card.Title class="text-base">สถานศึกษาเดิม</Card.Title>
						{#if scopeKey === 'overall'}
							<Card.Description>
								โรงเรียนตนเอง: {reportConfig?.institution?.ownSchool || '(ยังไม่ได้กำหนด)'}
							</Card.Description>
						{/if}
					</Card.Header>
					<Card.Content>
						<div class="overflow-x-auto">
							<table class="w-full text-sm min-w-[320px]">
								<thead>
									<tr class="border-b">
										<th class="text-left py-2 font-medium text-muted-foreground">ประเภท</th>
										<th class="text-right py-2 font-medium text-muted-foreground w-16">จำนวน</th>
										<th class="text-right py-2 font-medium text-muted-foreground w-16">%</th>
										<th class="py-2 w-24 hidden sm:table-cell"></th>
									</tr>
								</thead>
								<tbody>
									<!-- โรงเรียนตนเอง (ไม่ expandable) -->
									<tr class="border-b">
										<td class="py-2">สถานศึกษาเดิม (โรงเรียนตนเอง)</td>
										<td class="text-right py-2 tabular-nums">{inst.fromOwn.total}</td>
										<td class="text-right py-2 tabular-nums">{pct(inst.fromOwn.total, inst.total)}</td>
										<td class="py-2 pl-3 hidden sm:table-cell">
											<div class="h-2 rounded-full bg-muted overflow-hidden">
												<div class="h-full bg-blue-500 rounded-full" style="width: {inst.total ? (inst.fromOwn.total/inst.total*100) : 0}%"></div>
											</div>
										</td>
									</tr>

									<!-- สถานศึกษาอื่น — expandable -->
									<tr
										class="border-b {inst.fromOther.schools.length > 0 ? 'cursor-pointer hover:bg-muted/40' : ''}"
										onclick={() => inst.fromOther.schools.length > 0 && toggleExpand(`${scopeKey}-fromOther`)}
									>
										<td class="py-2">
											<span class="flex items-center gap-1.5">
												สถานศึกษาอื่น
												{#if inst.fromOther.schools.length > 0}
													<ChevronDown class="w-3.5 h-3.5 text-muted-foreground transition-transform duration-200 {isExpanded(`${scopeKey}-fromOther`) ? 'rotate-180' : ''}" />
												{/if}
											</span>
										</td>
										<td class="text-right py-2 tabular-nums">{inst.fromOther.total}</td>
										<td class="text-right py-2 tabular-nums">{pct(inst.fromOther.total, inst.total)}</td>
										<td class="py-2 pl-3 hidden sm:table-cell">
											<div class="h-2 rounded-full bg-muted overflow-hidden">
												<div class="h-full bg-orange-400 rounded-full" style="width: {inst.total ? (inst.fromOther.total/inst.total*100) : 0}%"></div>
											</div>
										</td>
									</tr>
									{#if isExpanded(`${scopeKey}-fromOther`) && inst.fromOther.schools.length > 0}
										<tr>
											<td colspan="4" class="pb-3 pt-0 pl-5">
												<div class="space-y-0.5 text-xs">
													{#each inst.fromOther.schools as school}
														<div class="flex items-center justify-between pr-2 py-0.5 text-muted-foreground">
															<span class="flex items-center gap-1.5"><span class="opacity-40">└</span>{school.name}</span>
															<span class="tabular-nums shrink-0 ml-4">{school.count} <span class="opacity-60">({pct(school.count, inst.fromOther.total)})</span></span>
														</div>
													{/each}
												</div>
											</td>
										</tr>
									{/if}

									<!-- ไม่ระบุ -->
									<tr class="border-b">
										<td class="py-2 text-muted-foreground">ไม่ระบุ</td>
										<td class="text-right py-2 tabular-nums">{inst.noInfo.total}</td>
										<td class="text-right py-2 tabular-nums">{pct(inst.noInfo.total, inst.total)}</td>
										<td class="py-2 pl-3 hidden sm:table-cell">
											<div class="h-2 rounded-full bg-muted overflow-hidden">
												<div class="h-full bg-gray-400 rounded-full" style="width: {inst.total ? (inst.noInfo.total/inst.total*100) : 0}%"></div>
											</div>
										</td>
									</tr>
									<tr class="font-semibold">
										<td class="py-2">รวม</td>
										<td class="text-right py-2 tabular-nums">{inst.total}</td>
										<td class="text-right py-2 tabular-nums">100%</td>
										<td class="hidden sm:table-cell"></td>
									</tr>
								</tbody>
							</table>
						</div>
					</Card.Content>
				</Card.Root>
			{/if}
		</div>
	{/if}
{/snippet}

{#if loading}
	<div class="flex justify-center items-center py-20">
		<LoaderCircle class="w-8 h-8 animate-spin text-primary" />
	</div>
{:else if round}
	<div class="space-y-6">
		<!-- Header -->
		<div class="flex items-center justify-between">
			<Button href="/staff/academic/admission/{id}" variant="ghost" size="sm" class="gap-1">
				<ArrowLeft class="w-4 h-4" /> ย้อนกลับ
			</Button>
		</div>

		<div>
			<h1 class="text-2xl font-bold">รายงานการรับสมัคร</h1>
			<p class="text-muted-foreground">{round.name}</p>
		</div>

		{#if !reportConfig || reportConfig.reportMode === null}
			<Card.Root>
				<Card.Content class="py-12 text-center space-y-3">
					<p class="text-muted-foreground">ยังไม่ได้ตั้งค่าการรายงานสำหรับรอบนี้</p>
					<Button href="/staff/academic/admission/{id}" variant="outline" size="sm" class="gap-1.5">
						<Settings class="w-3.5 h-3.5" /> ไปตั้งค่า
					</Button>
				</Card.Content>
			</Card.Root>
		{:else}
			<!-- Summary cards -->
			<div class="grid grid-cols-1 sm:grid-cols-3 gap-4">
				<Card.Root>
					<Card.Content class="p-4">
						<p class="text-xs text-muted-foreground">ประเภทการรายงาน</p>
						<p class="font-semibold mt-0.5">{reportModeLabel[reportConfig.reportMode]}</p>
					</Card.Content>
				</Card.Root>
				<Card.Root>
					<Card.Content class="p-4">
						<p class="text-xs text-muted-foreground">ผู้สมัครทั้งหมด</p>
						<p class="font-semibold mt-0.5">{applications.length} คน</p>
					</Card.Content>
				</Card.Root>
				<Card.Root>
					<Card.Content class="p-4">
						<p class="text-xs text-muted-foreground">แสดง (ตามตัวกรอง)</p>
						<p class="font-semibold mt-0.5">{filteredApps.length} คน</p>
					</Card.Content>
				</Card.Root>
			</div>

			<!-- Status filter -->
			<div class="flex flex-col sm:flex-row items-start sm:items-center gap-2">
				<span class="text-sm font-medium">กรอง:</span>
				<Select.Root type="single" bind:value={statusFilter}>
					<Select.Trigger class="w-full sm:w-48">
						{statusFilter === 'all' ? 'สถานะทั้งหมด' : (applicationStatusLabel[statusFilter] ?? statusFilter)}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="all">สถานะทั้งหมด</Select.Item>
						{#each allStatuses as s}
							<Select.Item value={s}>{applicationStatusLabel[s] ?? s}</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>

			<!-- รายงานรวม -->
			<div class="space-y-3">
				<h2 class="text-base font-semibold border-b pb-2">รายงานรวม</h2>
				{@render statsBlock('overall', filteredApps)}
			</div>

			<!-- แยกตามวัน -->
			<div class="space-y-3">
				<h2 class="text-base font-semibold border-b pb-2">แยกตามวัน</h2>
				{#if dayGroups().length === 0}
					<p class="text-sm text-muted-foreground">ไม่มีข้อมูล</p>
				{:else}
					<div class="space-y-2">
						{#each dayGroups() as group (group.date)}
							<Card.Root>
								<button
									type="button"
									class="w-full text-left px-5 py-3.5 flex items-center justify-between hover:bg-muted/40 transition-colors rounded-xl"
									onclick={() => toggleExpand(`day-${group.date}`)}
								>
									<span class="font-medium text-sm">
										{formatThaiDate(group.date)}
										<span class="text-muted-foreground font-normal ml-1.5">({group.apps.length} คน)</span>
									</span>
									<ChevronDown class="w-4 h-4 text-muted-foreground transition-transform duration-200 shrink-0 {isExpanded(`day-${group.date}`) ? 'rotate-180' : ''}" />
								</button>
								{#if isExpanded(`day-${group.date}`)}
									<div class="px-5 pb-4 space-y-4 border-t border-border pt-4">
										{@render statsBlock(group.date, group.apps)}
									</div>
								{/if}
							</Card.Root>
						{/each}
					</div>
				{/if}
			</div>
		{/if}
	</div>
{/if}
