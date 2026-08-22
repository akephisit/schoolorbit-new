<script lang="ts">
	import type {
		CertificateCampaignStatus,
		CertificateCampaignSummary
	} from '$lib/api/certificates';
	import { PageState } from '$lib/components/app-state';
	import CertificateCampaignPurgeDialog from '$lib/components/certificates/CertificateCampaignPurgeDialog.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
	import {
		Award,
		Building2,
		CalendarDays,
		ChevronRight,
		FileBadge2,
		LoaderCircle,
		Search,
		UsersRound
	} from 'lucide-svelte';

	let {
		campaigns,
		canCreate = false,
		onpurged
	}: {
		campaigns: CertificateCampaignSummary[];
		canCreate?: boolean;
		onpurged?: (campaignId: string) => void;
	} = $props();

	let search = $state('');
	let statusFilter = $state('all');
	let academicYearFilter = $state('all');
	let purgeCampaign: CertificateCampaignSummary | null = $state.raw(null);

	const statusLabels: Record<CertificateCampaignStatus, string> = {
		draft: 'ฉบับร่าง',
		active: 'กำลังออก',
		closed: 'ปิดกิจกรรม',
		archived: 'เก็บถาวร',
		purging: 'กำลังลบ'
	};

	const statusClasses: Record<CertificateCampaignStatus, string> = {
		draft: 'border-slate-200 bg-slate-50 text-slate-700',
		active: 'border-emerald-200 bg-emerald-50 text-emerald-700',
		closed: 'border-amber-200 bg-amber-50 text-amber-700',
		archived: 'border-zinc-200 bg-zinc-100 text-zinc-600',
		purging: 'border-red-200 bg-red-50 text-red-700'
	};

	const statusRailClasses: Record<CertificateCampaignStatus, string> = {
		draft: 'bg-slate-300',
		active: 'bg-emerald-500',
		closed: 'bg-amber-500',
		archived: 'bg-zinc-400',
		purging: 'bg-red-500'
	};

	const academicYears = $derived.by(() => {
		const values: Array<[string, string]> = [];
		for (const campaign of campaigns) {
			if (!values.some(([id]) => id === campaign.academicYearId)) {
				values.push([campaign.academicYearId, campaign.academicYearName]);
			}
		}
		return values.sort((left, right) => right[1].localeCompare(left[1], 'th'));
	});

	const filteredCampaigns = $derived.by(() => {
		const normalizedSearch = search.trim().toLocaleLowerCase('th');
		return campaigns.filter((campaign) => {
			const matchesSearch =
				!normalizedSearch ||
				campaign.name.toLocaleLowerCase('th').includes(normalizedSearch) ||
				campaign.ownerOrganizationUnitName?.toLocaleLowerCase('th').includes(normalizedSearch);
			const matchesStatus = statusFilter === 'all' || campaign.status === statusFilter;
			const matchesYear =
				academicYearFilter === 'all' || campaign.academicYearId === academicYearFilter;
			return matchesSearch && matchesStatus && matchesYear;
		});
	});

	function formatDate(value: string): string {
		return new Date(`${value}T00:00:00`).toLocaleDateString('th-TH', {
			day: 'numeric',
			month: 'short',
			year: 'numeric'
		});
	}
</script>

<div class="space-y-4">
	<div
		class="grid gap-3 rounded-xl border bg-card p-3 lg:grid-cols-[minmax(16rem,1fr)_13rem_13rem]"
	>
		<label class="relative block">
			<span class="sr-only">ค้นหากิจกรรมหรือหน่วยงาน</span>
			<Search
				class="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
			/>
			<Input bind:value={search} class="pl-9" placeholder="ค้นหากิจกรรมหรือหน่วยงาน" />
		</label>

		<Select.Root type="single" bind:value={academicYearFilter}>
			<Select.Trigger class="w-full" aria-label="กรองตามปีการศึกษา">
				{academicYearFilter === 'all'
					? 'ทุกปีการศึกษา'
					: (academicYears.find(([id]) => id === academicYearFilter)?.[1] ?? 'ปีการศึกษา')}
			</Select.Trigger>
			<Select.Content>
				<Select.Item value="all">ทุกปีการศึกษา</Select.Item>
				{#each academicYears as [id, name] (id)}
					<Select.Item value={id}>{name}</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>

		<Select.Root type="single" bind:value={statusFilter}>
			<Select.Trigger class="w-full" aria-label="กรองตามสถานะ">
				{statusFilter === 'all'
					? 'ทุกสถานะ'
					: statusLabels[statusFilter as CertificateCampaignStatus]}
			</Select.Trigger>
			<Select.Content>
				<Select.Item value="all">ทุกสถานะ</Select.Item>
				<Select.Item value="draft">ฉบับร่าง</Select.Item>
				<Select.Item value="active">กำลังออก</Select.Item>
				<Select.Item value="closed">ปิดกิจกรรม</Select.Item>
				<Select.Item value="archived">เก็บถาวร</Select.Item>
				<Select.Item value="purging">กำลังลบ</Select.Item>
			</Select.Content>
		</Select.Root>
	</div>

	{#if filteredCampaigns.length === 0}
		<PageState
			title={campaigns.length === 0 ? 'ยังไม่มีชุดออกเกียรติบัตร' : 'ไม่พบกิจกรรมที่ค้นหา'}
			description={campaigns.length === 0
				? 'สร้างกิจกรรมเพื่อเริ่มออกแบบแม่แบบและเตรียมรายชื่อผู้รับ'
				: 'ลองเปลี่ยนคำค้น ปีการศึกษา หรือสถานะ'}
			actionLabel={campaigns.length === 0 && canCreate ? 'สร้างกิจกรรมแรก' : undefined}
			href={campaigns.length === 0 && canCreate ? '/staff/certificates/new' : undefined}
		/>
	{:else}
		<div class="overflow-hidden rounded-xl border bg-card">
			{#each filteredCampaigns as campaign, index (campaign.id)}
				<article
					class="group relative grid gap-4 p-4 pl-5 transition-colors hover:bg-muted/30 md:grid-cols-[minmax(0,1fr)_auto] md:items-center lg:p-5 lg:pl-6"
					class:border-b={index < filteredCampaigns.length - 1}
				>
					<div class={`absolute inset-y-0 left-0 w-1 ${statusRailClasses[campaign.status]}`}></div>

					<div class="min-w-0 space-y-3">
						<div class="flex flex-wrap items-center gap-2">
							<h2 class="truncate text-base font-semibold text-foreground lg:text-lg">
								{campaign.name}
							</h2>
							<Badge variant="outline" class={statusClasses[campaign.status]}>
								{statusLabels[campaign.status]}
							</Badge>
							{#if campaign.activitySequence !== null}
								<Badge variant="secondary">กิจกรรมลำดับ {campaign.activitySequence}</Badge>
							{/if}
							{#if campaign.hasOpenIssueRequest}
								<Badge variant="outline" class="border-blue-200 bg-blue-50 text-blue-700">
									มีคำขอรอตรวจ
								</Badge>
							{/if}
						</div>

						<div class="flex flex-wrap gap-x-5 gap-y-2 text-sm text-muted-foreground">
							<span class="inline-flex items-center gap-1.5">
								<CalendarDays class="size-4" />
								{campaign.academicYearName} · {formatDate(campaign.eventDate)}
							</span>
							<span class="inline-flex items-center gap-1.5">
								<Building2 class="size-4" />
								{campaign.ownerOrganizationUnitName ?? 'กิจกรรมระดับโรงเรียน'}
							</span>
						</div>

						<div class="flex flex-wrap gap-4 text-xs text-muted-foreground">
							<span class="inline-flex items-center gap-1.5">
								<FileBadge2 class="size-3.5" />
								{campaign.templateCount} แบบ
							</span>
							<span class="inline-flex items-center gap-1.5">
								<UsersRound class="size-3.5" />
								{campaign.candidateCount} รายชื่อ
							</span>
							<span class="inline-flex items-center gap-1.5">
								<Award class="size-3.5" /> ออกแล้ว {campaign.issuedCertificateCount} ใบ
							</span>
						</div>
					</div>

					{#if campaign.status === 'purging'}
						<Button
							variant="outline"
							class="w-full justify-between border-red-200 text-red-700 hover:bg-red-50 hover:text-red-800 md:w-auto"
							onclick={() => (purgeCampaign = campaign)}
						>
							<span class="inline-flex items-center gap-2">
								<LoaderCircle class="size-4 animate-spin" /> ดูสถานะการลบ
							</span>
							<ChevronRight class="size-4" />
						</Button>
					{:else}
						<Button
							href={`/staff/certificates/${campaign.id}/overview`}
							variant="ghost"
							class="w-full justify-between md:w-auto"
						>
							เปิดชุดออก
							<ChevronRight class="size-4 transition-transform group-hover:translate-x-0.5" />
						</Button>
					{/if}
				</article>
			{/each}
		</div>
	{/if}
</div>

{#if purgeCampaign}
	<CertificateCampaignPurgeDialog
		open={true}
		campaignId={purgeCampaign.id}
		campaignName={purgeCampaign.name}
		initiallyPurging={true}
		onopenchange={(open) => {
			if (!open) purgeCampaign = null;
		}}
		oncompleted={() => {
			if (!purgeCampaign) return;
			const completedId = purgeCampaign.id;
			purgeCampaign = null;
			onpurged?.(completedId);
		}}
	/>
{/if}
