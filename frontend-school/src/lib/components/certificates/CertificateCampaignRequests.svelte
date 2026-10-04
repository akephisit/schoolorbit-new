<script lang="ts">
	import { resolve } from '$app/paths';
	import {
		listCertificateCampaignIssueRequests,
		withdrawCertificateIssueRequest,
		type CertificateIssueCode,
		type CertificateIssueRequestStatus,
		type CertificateIssueRequestSummary
	} from '#lib/api/certificates.js';
	import { LoadingButton, PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import {
		ArrowLeft,
		CircleCheckBig,
		Clock3,
		FileClock,
		RotateCcw,
		ShieldCheck,
		Undo2
	} from '@lucide/svelte';
	import { onDestroy, untrack } from 'svelte';
	import { LatestRequest } from '#lib/async/latest-request.js';
	import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
	import { toast } from 'svelte-sonner';

	let {
		campaignId,
		identityKey,
		initialRequests,
		canRead,
		canSubmit
	}: {
		campaignId: string;
		identityKey: string;
		initialRequests: Promise<
			RouteLoadResult<{ ownerKey: string; records: CertificateIssueRequestSummary[] | null }>
		>;
		canRead: boolean;
		canSubmit: boolean;
	} = $props();

	const statusMeta: Record<
		CertificateIssueRequestStatus,
		{ label: string; className: string; description: string }
	> = {
		pending: {
			label: 'รอตรวจ',
			className: 'border-blue-200 bg-blue-50 text-blue-800',
			description: 'ส่งถึงคิวส่วนกลางแล้ว'
		},
		reviewing: {
			label: 'กำลังตรวจ',
			className: 'border-amber-200 bg-amber-50 text-amber-800',
			description: 'ผู้ตรวจรับคำขอแล้ว'
		},
		returned: {
			label: 'ส่งกลับแล้ว',
			className: 'border-orange-200 bg-orange-50 text-orange-800',
			description: 'แก้รายการแล้วส่งเป็นคำขอใหม่'
		},
		withdrawn: {
			label: 'ถอนแล้ว',
			className: 'border-slate-200 bg-slate-50 text-slate-700',
			description: 'คำขอนี้สิ้นสุดโดยผู้ส่ง'
		},
		issued: {
			label: 'ออกแล้ว',
			className: 'border-emerald-200 bg-emerald-50 text-emerald-800',
			description: 'ออกเลขเกียรติบัตรเรียบร้อย'
		}
	};

	const issueLabels: Record<CertificateIssueCode, string> = {
		candidate_not_ready: 'ข้อมูลผู้รับไม่พร้อม',
		account_state_changed: 'สถานะบัญชีเปลี่ยน',
		template_not_ready: 'แบบยังไม่พร้อม',
		template_incompatible: 'แบบไม่รองรับผู้รับ',
		asset_unavailable: 'ไฟล์ในแบบไม่พร้อม',
		campaign_unavailable: 'กิจกรรมไม่พร้อม',
		reviewer_requested_changes: 'ผู้ตรวจขอให้แก้ไข'
	};

	let requests = $state.raw<CertificateIssueRequestSummary[]>([]);
	let loading = $state(true);
	let error = $state('');
	let withdrawingId = $state<string | null>(null);

	const context = $derived(`${identityKey}|${campaignId}`),
		readRequest = new LatestRequest();
	let owner = $state(''),
		ownerEpoch = $state(0),
		disposed = false,
		loaded = $state(false);
	let consumed: typeof initialRequests | null = null;
	$effect.pre(() => {
		const key = context,
			source = initialRequests,
			allowed = canRead;
		untrack(() => {
			if (owner !== key) {
				owner = key;
				ownerEpoch++;
				readRequest.abort();
				requests = [];
				withdrawingId = null;
				loaded = false;
				loading = allowed;
				error = '';
			}
			if (!allowed || consumed === source) return;
			consumed = source;
			const t = readRequest.begin();
			loading = true;
			error = '';
			void source.then((r) => applyPrimary(r, t.revision));
		});
	});
	onDestroy(() => {
		disposed = true;
		ownerEpoch++;
		readRequest.abort();
	});
	function currentOwner(epoch: number, key: string) {
		return !disposed && epoch === ownerEpoch && key === context && canRead;
	}
	function applyPrimary(r: Awaited<typeof initialRequests>, revision: number) {
		if (!readRequest.isCurrent(revision)) return;
		loading = false;
		if (!r.ok) {
			error = r.error;
			return;
		}
		if (r.data.ownerKey !== context) return;
		requests = r.data.records ?? [];
		loaded = true;
	}

	function formatDate(value: string): string {
		return new Intl.DateTimeFormat('th-TH', {
			dateStyle: 'medium',
			timeStyle: 'short',
			timeZone: 'Asia/Bangkok'
		}).format(new Date(value));
	}

	async function loadRequests(targetCampaignId: string) {
		if (disposed || !canRead || targetCampaignId !== campaignId) return;
		const ownerKey = context,
			t = readRequest.begin();
		loading = true;
		error = '';
		applyPrimary(
			await captureRouteLoad(
				listCertificateCampaignIssueRequests(targetCampaignId, { signal: t.signal }).then(
					(records) => ({ ownerKey, records })
				),
				'โหลดข้อมูลเกียรติบัตรไม่สำเร็จ'
			),
			t.revision
		);
	}

	async function withdraw(requestId: string) {
		if (
			withdrawingId ||
			!canSubmit ||
			!requests.some((x) => x.id === requestId && x.capabilities.canWithdraw)
		)
			return;
		const epoch = ownerEpoch,
			key = context;
		withdrawingId = requestId;
		try {
			const updated = await withdrawCertificateIssueRequest(requestId);
			if (!currentOwner(epoch, key) || !canSubmit) return;
			readRequest.abort();
			loading = false;
			requests = requests.map((request) => (request.id === requestId ? updated : request));
			toast.success('ถอนคำขอแล้ว');
		} catch (withdrawError) {
			if (!currentOwner(epoch, key) || !canSubmit) return;
			toast.error(withdrawError instanceof Error ? withdrawError.message : 'ถอนคำขอไม่สำเร็จ');
		} finally {
			if (currentOwner(epoch, key)) withdrawingId = null;
		}
	}
</script>

<PageShell
	title="ประวัติคำขอออกเกียรติบัตร"
	description="ติดตามคำขอที่ส่งให้ส่วนกลาง แต่ละคำขอเก็บเป็นประวัติและไม่เขียนทับคำขอเดิม"
>
	{#snippet meta()}
		<div class="flex items-center gap-2 text-xs text-muted-foreground">
			<FileClock class="size-4" />
			{requests.length.toLocaleString('th-TH')} คำขอ
		</div>
	{/snippet}

	{#snippet actions()}
		<Button
			data-sveltekit-preload-data="tap"
			variant="outline"
			href={resolve(`staff/certificates/${campaignId}/recipients`)}
		>
			<ArrowLeft class="size-4" /> กลับไปเตรียมรายชื่อ
		</Button>
	{/snippet}

	{#if !canRead}
		<PageState
			variant="permission"
			title="ไม่มีสิทธิ์ดูคำขอออกเกียรติบัตร"
			description="หน้านี้ใช้สิทธิ์อ่านในขอบเขตหน่วยงานหรือทั้งโรงเรียน"
		/>
	{:else}
		{#if loading && loaded}<p role="status">กำลังอัปเดตประวัติคำขอ</p>{/if}
		{#if error}
			<PageState
				variant="error"
				title="โหลดประวัติคำขอไม่สำเร็จ"
				description={error}
				actionLabel="ลองอีกครั้ง"
				onaction={() => loadRequests(campaignId)}
			/>
		{/if}
		{#if loading && !loaded}<div role="status" aria-label="กำลังโหลดประวัติคำขอ">
				<PageSkeleton variant="cards" />
			</div>
		{:else if loaded && requests.length === 0}
			<PageState
				variant="empty"
				title="ยังไม่มีคำขอในกิจกรรมนี้"
				description="เลือกรายชื่อสถานะพร้อมออกจากหน้ารายชื่อผู้รับ แล้วส่งให้ส่วนกลางตรวจ"
			/>
		{:else if loaded}
			<div
				class="relative space-y-4 before:absolute before:inset-y-5 before:left-[21px] before:w-px before:bg-border"
			>
				{#each requests as request (request.id)}
					{@const meta = statusMeta[request.status]}
					<article
						id={`request-${request.id}`}
						class="relative grid scroll-mt-20 gap-4 pl-12 sm:grid-cols-[1fr_auto]"
					>
						<div
							class="absolute left-3 top-5 z-10 flex size-[19px] items-center justify-center rounded-full border bg-background"
						>
							{#if request.status === 'issued'}
								<CircleCheckBig class="size-3 text-emerald-600" />
							{:else if request.status === 'returned'}
								<RotateCcw class="size-3 text-orange-600" />
							{:else if request.status === 'withdrawn'}
								<Undo2 class="size-3 text-slate-500" />
							{:else if request.status === 'reviewing'}
								<ShieldCheck class="size-3 text-amber-600" />
							{:else}
								<Clock3 class="size-3 text-blue-600" />
							{/if}
						</div>

						<div class="rounded-xl border bg-card p-5 shadow-sm">
							<div class="flex flex-wrap items-start justify-between gap-3">
								<div>
									<div class="flex flex-wrap items-center gap-2">
										<Badge variant="outline" class={meta.className}>{meta.label}</Badge>
										<span class="text-xs text-muted-foreground">{meta.description}</span>
									</div>
									<p class="mt-2 font-medium">ส่งโดย {request.submittedByName}</p>
									<p class="mt-1 text-xs text-muted-foreground">
										{formatDate(request.submittedAt)} · {request.itemCount.toLocaleString('th-TH')}
										รายชื่อ · {request.templateCount.toLocaleString('th-TH')} แบบ
									</p>
								</div>
								{#if canSubmit && request.capabilities.canWithdraw}
									<LoadingButton
										variant="outline"
										size="sm"
										loading={withdrawingId === request.id}
										disabled={withdrawingId !== null && withdrawingId !== request.id}
										onclick={() => withdraw(request.id)}
									>
										<Undo2 class="size-4" /> ถอนคำขอ
									</LoadingButton>
								{/if}
							</div>

							{#if request.status === 'returned'}
								<div
									class="mt-4 rounded-lg border border-orange-200 bg-orange-50 p-4 text-sm text-orange-950"
								>
									<p class="font-medium">เหตุผลที่ส่งกลับ</p>
									<div class="mt-2 flex flex-wrap gap-1.5">
										{#each request.issueCodes as code (code)}
											<Badge variant="outline" class="border-orange-300 bg-white text-orange-900">
												{issueLabels[code]}
											</Badge>
										{/each}
									</div>
									{#if request.returnNote}<p class="mt-2">{request.returnNote}</p>{/if}
								</div>
							{/if}
						</div>
					</article>
				{/each}
			</div>
		{/if}
	{/if}
</PageShell>
