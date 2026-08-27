<script lang="ts">
	import { afterNavigate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import {
		listCertificateIssueRequests,
		type CertificateIssueRequestStatus,
		type CertificateIssueRequestSummary
	} from '$lib/api/certificates';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Select from '$lib/components/ui/select';
	import * as Table from '$lib/components/ui/table';
	import { AlertTriangle, ArrowRight, RefreshCw, ShieldCheck } from 'lucide-svelte';
	import { onMount } from 'svelte';

	let { canIssue }: { canIssue: boolean } = $props();

	type StatusFilter = CertificateIssueRequestStatus | 'all';

	const statusMeta: Record<CertificateIssueRequestStatus, { label: string; className: string }> = {
		pending: { label: 'รอตรวจ', className: 'border-blue-200 bg-blue-50 text-blue-800' },
		reviewing: { label: 'กำลังตรวจ', className: 'border-amber-200 bg-amber-50 text-amber-800' },
		returned: { label: 'ส่งกลับแล้ว', className: 'border-orange-200 bg-orange-50 text-orange-800' },
		withdrawn: { label: 'ถอนแล้ว', className: 'border-slate-200 bg-slate-50 text-slate-700' },
		issued: { label: 'ออกแล้ว', className: 'border-emerald-200 bg-emerald-50 text-emerald-800' }
	};

	let requests = $state.raw<CertificateIssueRequestSummary[]>([]);
	let statusFilter = $state<StatusFilter>('all');
	let loading = $state(true);
	let error = $state('');
	let loadGeneration = 0;
	let initialized = false;

	function formatDate(value: string): string {
		return new Intl.DateTimeFormat('th-TH', {
			dateStyle: 'medium',
			timeStyle: 'short',
			timeZone: 'Asia/Bangkok'
		}).format(new Date(value));
	}

	async function loadQueue() {
		const generation = ++loadGeneration;
		if (!canIssue) {
			loading = false;
			return;
		}
		loading = true;
		error = '';
		try {
			const loaded = await listCertificateIssueRequests({
				status: statusFilter === 'all' ? undefined : statusFilter
			});
			if (generation !== loadGeneration) return;
			requests = loaded;
		} catch (loadError) {
			if (generation !== loadGeneration) return;
			error = loadError instanceof Error ? loadError.message : 'โหลดคิวคำขอไม่สำเร็จ';
		} finally {
			if (generation === loadGeneration) loading = false;
		}
	}

	function changeStatusFilter(value: string): void {
		if (
			value !== 'all' &&
			value !== 'pending' &&
			value !== 'reviewing' &&
			value !== 'returned' &&
			value !== 'withdrawn' &&
			value !== 'issued'
		)
			return;
		statusFilter = value;
		void loadQueue();
	}

	function initialize() {
		if (initialized) return;
		initialized = true;
		void loadQueue();
	}

	onMount(initialize);
	afterNavigate(initialize);
</script>

<PageShell
	title="คิวตรวจคำขอออกเกียรติบัตร"
	description="ตรวจคำขอจากทุกหน่วยงานก่อนออกเลขจริง รายชื่อผู้รับจะแสดงเมื่อเปิดคำขอเท่านั้น"
>
	{#snippet meta()}
		<div class="flex items-center gap-2 text-xs text-muted-foreground">
			<ShieldCheck class="size-4" /> สิทธิ์ออกเกียรติบัตรระดับโรงเรียน
		</div>
	{/snippet}

	{#snippet actions()}
		<Button variant="outline" disabled={loading || !canIssue} onclick={loadQueue}>
			<RefreshCw class={`size-4 ${loading ? 'animate-spin' : ''}`} /> โหลดใหม่
		</Button>
	{/snippet}

	{#if !canIssue}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ตรวจคำขอออกเกียรติบัตร"
			description="คิวนี้ใช้สิทธิ์ออกเกียรติบัตรระดับโรงเรียนเท่านั้น"
		/>
	{:else}
		<section
			class="mb-4 flex flex-wrap items-end justify-between gap-3 rounded-xl border bg-card p-4"
		>
			<div>
				<p class="text-sm font-semibold">ถาดงานตรวจ</p>
				<p class="mt-1 text-xs text-muted-foreground">เรียงคำขอรอตรวจก่อน ตามเวลาที่หน่วยงานส่ง</p>
			</div>
			<label class="min-w-52 space-y-1.5">
				<span class="text-xs font-medium text-muted-foreground">สถานะคำขอ</span>
				<Select.Root type="single" value={statusFilter} onValueChange={changeStatusFilter}>
					<Select.Trigger class="w-full">
						{statusFilter === 'all' ? 'ทุกสถานะ' : statusMeta[statusFilter].label}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="all">ทุกสถานะ</Select.Item>
						<Select.Item value="pending">รอตรวจ</Select.Item>
						<Select.Item value="reviewing">กำลังตรวจ</Select.Item>
						<Select.Item value="returned">ส่งกลับแล้ว</Select.Item>
						<Select.Item value="withdrawn">ถอนแล้ว</Select.Item>
						<Select.Item value="issued">ออกแล้ว</Select.Item>
					</Select.Content>
				</Select.Root>
			</label>
		</section>

		{#if loading}
			<PageSkeleton variant="table" />
		{:else if error}
			<PageState
				variant="error"
				title="โหลดคิวคำขอไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={loadQueue}
			/>
		{:else if requests.length === 0}
			<PageState
				variant="empty"
				title="ไม่มีคำขอในสถานะนี้"
				description="เมื่อหน่วยงานส่งรายชื่อพร้อมออก คำขอจะปรากฏในถาดงานนี้"
			/>
		{:else}
			<div class="overflow-x-auto rounded-xl border bg-card shadow-sm">
				<Table.Root class="min-w-[1040px]">
					<Table.Header>
						<Table.Row class="bg-muted/40 hover:bg-muted/40">
							<Table.Head class="w-36">สถานะ</Table.Head>
							<Table.Head class="w-56">กิจกรรม / หน่วยงาน</Table.Head>
							<Table.Head class="w-52">ผู้ส่ง</Table.Head>
							<Table.Head class="w-32 text-right">รายชื่อ / แบบ</Table.Head>
							<Table.Head class="w-64">สัญญาณความพร้อม</Table.Head>
							<Table.Head class="w-48">เวลาที่ส่ง</Table.Head>
							<Table.Head class="w-16"><span class="sr-only">เปิดคำขอ</span></Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each requests as request (request.id)}
							{@const meta = statusMeta[request.status]}
							<Table.Row>
								<Table.Cell class="align-top">
									<Badge variant="outline" class={meta.className}>{meta.label}</Badge>
								</Table.Cell>
								<Table.Cell class="align-top">
									<p class="font-medium">{request.campaignName}</p>
									<p class="mt-1 text-xs text-muted-foreground">
										{request.ownerOrganizationUnitName ?? 'ระดับโรงเรียน'}
									</p>
								</Table.Cell>
								<Table.Cell class="align-top">{request.submittedByName}</Table.Cell>
								<Table.Cell class="text-right align-top tabular-nums">
									<p>{request.itemCount.toLocaleString('th-TH')} รายชื่อ</p>
									<p class="mt-1 text-xs text-muted-foreground">
										{request.templateCount.toLocaleString('th-TH')} แบบ
									</p>
								</Table.Cell>
								<Table.Cell class="align-top">
									<div class="flex flex-wrap gap-1.5 text-xs">
										<Badge
											variant="outline"
											class="border-emerald-200 bg-emerald-50 text-emerald-800"
										>
											พร้อม {request.readyCount.toLocaleString('th-TH')}
										</Badge>
										{#if request.reviewCount > 0}
											<Badge variant="outline" class="border-amber-200 bg-amber-50 text-amber-800">
												<AlertTriangle class="size-3" /> ตรวจ {request.reviewCount.toLocaleString(
													'th-TH'
												)}
											</Badge>
										{/if}
										{#if request.invalidCount > 0}
											<Badge variant="outline" class="border-red-200 bg-red-50 text-red-800">
												ผิด {request.invalidCount.toLocaleString('th-TH')}
											</Badge>
										{/if}
									</div>
								</Table.Cell>
								<Table.Cell class="align-top text-sm text-muted-foreground">
									{formatDate(request.submittedAt)}
								</Table.Cell>
								<Table.Cell class="align-top">
									<Button
										size="icon-sm"
										variant="ghost"
										href={resolve(
											`/staff/certificate-requests/${request.id}` as '/staff/certificate-requests/[requestId]'
										)}
										aria-label={`เปิดคำขอ ${request.campaignName}`}
									>
										<ArrowRight class="size-4" />
									</Button>
								</Table.Cell>
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
			</div>
		{/if}
	{/if}
</PageShell>
