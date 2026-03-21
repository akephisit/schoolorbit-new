<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import {
		getRound,
		listApplications,
		type AdmissionRound,
		type ApplicationListItem,
		type ReportConfig,
		applicationStatusLabel,
		roundStatusLabel
	} from '$lib/api/admission';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Select from '$lib/components/ui/select';
	import { Badge } from '$lib/components/ui/badge';
	import { Loader2, ArrowLeft, Settings, X } from 'lucide-svelte';
	import DatePicker from '$lib/components/ui/date-picker/DatePicker.svelte';
	import { toast } from 'svelte-sonner';

	let { data } = $props();

	let id = $derived($page.params.id);
	let round: AdmissionRound | null = $state(null);
	let applications: ApplicationListItem[] = $state([]);
	let loading = $state(true);
	let statusFilter = $state('all');
	let dateFilter = $state('');

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

	let reportConfig = $derived(round ? ((round as AdmissionRound).reportConfig ?? null) : null);

	let filteredApps = $derived(
		applications
			.filter((a) => !dateFilter || a.createdAt?.slice(0, 10) === dateFilter)
			.filter((a) => statusFilter === 'all' || a.status === statusFilter)
	);

	// Zone stats
	let zoneStats = $derived(() => {
		if (!reportConfig || (reportConfig.reportMode !== 'zone' && reportConfig.reportMode !== 'both')) return null;
		const schools = reportConfig.zone?.schools ?? [];
		const inZone = filteredApps.filter((a) => a.previousSchool && schools.includes(a.previousSchool)).length;
		const noInfo = filteredApps.filter((a) => !a.previousSchool).length;
		const outZone = filteredApps.length - inZone - noInfo;
		const total = filteredApps.length;
		return { inZone, outZone, noInfo, total };
	});

	// Institution stats
	let institutionStats = $derived(() => {
		if (!reportConfig || (reportConfig.reportMode !== 'institution' && reportConfig.reportMode !== 'both')) return null;
		const ownSchool = reportConfig.institution?.ownSchool ?? '';
		const fromOwn = filteredApps.filter((a) => a.previousSchool && a.previousSchool === ownSchool).length;
		const noInfo = filteredApps.filter((a) => !a.previousSchool).length;
		const fromOther = filteredApps.length - fromOwn - noInfo;
		const total = filteredApps.length;
		return { fromOwn, fromOther, noInfo, total };
	});

	function pct(n: number, total: number) {
		if (total === 0) return '0.0%';
		return ((n / total) * 100).toFixed(1) + '%';
	}

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

{#if loading}
	<div class="flex justify-center items-center py-20">
		<Loader2 class="w-8 h-8 animate-spin text-primary" />
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
			<!-- No config -->
			<Card.Root>
				<Card.Content class="py-12 text-center space-y-3">
					<p class="text-muted-foreground">ยังไม่ได้ตั้งค่าการรายงานสำหรับรอบนี้</p>
					<Button href="/staff/academic/admission/{id}" variant="outline" size="sm" class="gap-1.5">
						<Settings class="w-3.5 h-3.5" /> ไปตั้งค่า
					</Button>
				</Card.Content>
			</Card.Root>
		{:else}
			<!-- Summary card -->
			<div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-4">
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
						{#if dateFilter}
							<p class="text-xs text-muted-foreground mt-0.5">
								{new Date(dateFilter + 'T00:00:00').toLocaleDateString('th-TH', { year: 'numeric', month: 'short', day: 'numeric' })}
							</p>
						{/if}
					</Card.Content>
				</Card.Root>
			</div>

			<!-- Filters -->
			<div class="flex flex-col sm:flex-row flex-wrap items-start sm:items-center gap-2">
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
				<div class="flex items-center gap-1">
					<DatePicker bind:value={dateFilter} placeholder="กรองตามวันที่" class="w-full sm:w-44" />
					{#if dateFilter}
						<Button variant="ghost" size="icon" class="h-9 w-9 shrink-0" onclick={() => (dateFilter = '')} title="ล้างวันที่">
							<X class="w-3.5 h-3.5" />
						</Button>
					{/if}
				</div>
			</div>

			<!-- Zone section -->
			{#if reportConfig.reportMode === 'zone' || reportConfig.reportMode === 'both'}
				{@const z = zoneStats()}
				{#if z}
					<Card.Root>
						<Card.Header class="pb-2">
							<Card.Title class="text-base">เขตพื้นที่บริการ</Card.Title>
							<Card.Description>
								โรงเรียนในเขต: {reportConfig.zone?.schools?.join(', ') || '(ยังไม่ได้กำหนด)'}
							</Card.Description>
						</Card.Header>
						<Card.Content>
							<div class="overflow-x-auto">
							<table class="w-full text-sm min-w-[360px]">
								<thead>
									<tr class="border-b">
										<th class="text-left py-2 font-medium text-muted-foreground">ประเภท</th>
										<th class="text-right py-2 font-medium text-muted-foreground w-20">จำนวน</th>
										<th class="text-right py-2 font-medium text-muted-foreground w-20">%</th>
										<th class="py-2 w-32"></th>
									</tr>
								</thead>
								<tbody>
									<tr class="border-b">
										<td class="py-2">ในเขตพื้นที่</td>
										<td class="text-right py-2 tabular-nums">{z.inZone}</td>
										<td class="text-right py-2 tabular-nums">{pct(z.inZone, z.total)}</td>
										<td class="py-2 pl-3">
											<div class="h-2 rounded-full bg-muted overflow-hidden">
												<div class="h-full bg-green-500 rounded-full" style="width: {z.total ? (z.inZone/z.total*100) : 0}%"></div>
											</div>
										</td>
									</tr>
									<tr class="border-b">
										<td class="py-2">นอกเขตพื้นที่</td>
										<td class="text-right py-2 tabular-nums">{z.outZone}</td>
										<td class="text-right py-2 tabular-nums">{pct(z.outZone, z.total)}</td>
										<td class="py-2 pl-3">
											<div class="h-2 rounded-full bg-muted overflow-hidden">
												<div class="h-full bg-red-400 rounded-full" style="width: {z.total ? (z.outZone/z.total*100) : 0}%"></div>
											</div>
										</td>
									</tr>
									<tr class="border-b">
										<td class="py-2 text-muted-foreground">ไม่ระบุ</td>
										<td class="text-right py-2 tabular-nums">{z.noInfo}</td>
										<td class="text-right py-2 tabular-nums">{pct(z.noInfo, z.total)}</td>
										<td class="py-2 pl-3">
											<div class="h-2 rounded-full bg-muted overflow-hidden">
												<div class="h-full bg-gray-400 rounded-full" style="width: {z.total ? (z.noInfo/z.total*100) : 0}%"></div>
											</div>
										</td>
									</tr>
									<tr class="font-semibold">
										<td class="py-2">รวม</td>
										<td class="text-right py-2 tabular-nums">{z.total}</td>
										<td class="text-right py-2 tabular-nums">100%</td>
										<td></td>
									</tr>
								</tbody>
							</table>
						</div>
						</Card.Content>
					</Card.Root>
				{/if}
			{/if}

			<!-- Institution section -->
			{#if reportConfig.reportMode === 'institution' || reportConfig.reportMode === 'both'}
				{@const inst = institutionStats()}
				{#if inst}
					<Card.Root>
						<Card.Header class="pb-2">
							<Card.Title class="text-base">สถานศึกษาเดิม</Card.Title>
							<Card.Description>
								โรงเรียนตนเอง: {reportConfig.institution?.ownSchool || '(ยังไม่ได้กำหนด)'}
							</Card.Description>
						</Card.Header>
						<Card.Content>
							<div class="overflow-x-auto">
							<table class="w-full text-sm min-w-[360px]">
								<thead>
									<tr class="border-b">
										<th class="text-left py-2 font-medium text-muted-foreground">ประเภท</th>
										<th class="text-right py-2 font-medium text-muted-foreground w-20">จำนวน</th>
										<th class="text-right py-2 font-medium text-muted-foreground w-20">%</th>
										<th class="py-2 w-32"></th>
									</tr>
								</thead>
								<tbody>
									<tr class="border-b">
										<td class="py-2">สถานศึกษาเดิม (โรงเรียนตนเอง)</td>
										<td class="text-right py-2 tabular-nums">{inst.fromOwn}</td>
										<td class="text-right py-2 tabular-nums">{pct(inst.fromOwn, inst.total)}</td>
										<td class="py-2 pl-3">
											<div class="h-2 rounded-full bg-muted overflow-hidden">
												<div class="h-full bg-blue-500 rounded-full" style="width: {inst.total ? (inst.fromOwn/inst.total*100) : 0}%"></div>
											</div>
										</td>
									</tr>
									<tr class="border-b">
										<td class="py-2">สถานศึกษาอื่น</td>
										<td class="text-right py-2 tabular-nums">{inst.fromOther}</td>
										<td class="text-right py-2 tabular-nums">{pct(inst.fromOther, inst.total)}</td>
										<td class="py-2 pl-3">
											<div class="h-2 rounded-full bg-muted overflow-hidden">
												<div class="h-full bg-orange-400 rounded-full" style="width: {inst.total ? (inst.fromOther/inst.total*100) : 0}%"></div>
											</div>
										</td>
									</tr>
									<tr class="border-b">
										<td class="py-2 text-muted-foreground">ไม่ระบุ</td>
										<td class="text-right py-2 tabular-nums">{inst.noInfo}</td>
										<td class="text-right py-2 tabular-nums">{pct(inst.noInfo, inst.total)}</td>
										<td class="py-2 pl-3">
											<div class="h-2 rounded-full bg-muted overflow-hidden">
												<div class="h-full bg-gray-400 rounded-full" style="width: {inst.total ? (inst.noInfo/inst.total*100) : 0}%"></div>
											</div>
										</td>
									</tr>
									<tr class="font-semibold">
										<td class="py-2">รวม</td>
										<td class="text-right py-2 tabular-nums">{inst.total}</td>
										<td class="text-right py-2 tabular-nums">100%</td>
										<td></td>
									</tr>
								</tbody>
							</table>
						</div>
						</Card.Content>
					</Card.Root>
				{/if}
			{/if}
		{/if}
	</div>
{/if}
