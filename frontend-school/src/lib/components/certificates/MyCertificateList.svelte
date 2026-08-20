<script lang="ts">
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import {
		Award,
		CalendarDays,
		Download,
		ExternalLink,
		FileCheck2,
		LoaderCircle,
		RefreshCw,
		ShieldCheck,
		ShieldX
	} from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';
	import { PageShell } from '$lib/components/app-layout';
	import { PageState } from '$lib/components/app-state';
	import {
		createOwnCertificateRenderManifest,
		listOwnCertificates,
		type IssuedCertificateSummary
	} from '$lib/api/certificates';
	import { downloadCertificatePdf } from '$lib/certificates/download';
	import { loadCertificateRenderer } from '$lib/certificates/renderer';

	let {
		title = 'เกียรติบัตรที่ได้รับ',
		description = 'ใบที่โรงเรียนออกให้บัญชีนี้ พร้อมสถานะและหลักฐานตรวจสอบ'
	}: {
		title?: string;
		description?: string;
	} = $props();

	let certificates = $state.raw<IssuedCertificateSummary[]>([]);
	let loading = $state(true);
	let loadError = $state('');
	let downloadingId = $state<string | null>(null);
	let loadController: AbortController | null = null;

	const issuedCount = $derived(
		certificates.filter((certificate) => certificate.status === 'issued').length
	);
	const revokedCount = $derived(certificates.length - issuedCount);

	onMount(() => {
		void loadCertificates();
		return () => loadController?.abort();
	});

	async function loadCertificates(): Promise<void> {
		loadController?.abort();
		const controller = new AbortController();
		loadController = controller;
		loading = true;
		loadError = '';
		try {
			const loaded = await listOwnCertificates({ signal: controller.signal });
			if (controller.signal.aborted) return;
			certificates = loaded;
		} catch {
			if (controller.signal.aborted) return;
			certificates = [];
			loadError = 'โหลดคลังเกียรติบัตรไม่สำเร็จ';
		} finally {
			if (loadController === controller) loading = false;
		}
	}

	async function downloadCertificate(certificate: IssuedCertificateSummary): Promise<void> {
		if (
			certificate.status !== 'issued' ||
			certificate.capabilities.canDownload !== true ||
			downloadingId
		) {
			return;
		}
		downloadingId = certificate.id;
		try {
			const manifest = await createOwnCertificateRenderManifest(certificate.id);
			const renderer = await loadCertificateRenderer();
			const bytes = await renderer.buildCertificatePdf([manifest]);
			downloadCertificatePdf(bytes, manifest.suggestedFilename);
		} catch {
			toast.error('สร้างไฟล์ไม่สำเร็จ กรุณาโหลดรายการใหม่แล้วลองอีกครั้ง');
		} finally {
			downloadingId = null;
		}
	}

	function formatThaiDate(value: string): string {
		const [year, month, day] = value.split('-').map(Number);
		if (!year || !month || !day) return value;
		return new Intl.DateTimeFormat('th-TH', {
			dateStyle: 'long',
			timeZone: 'UTC'
		}).format(new Date(Date.UTC(year, month - 1, day)));
	}
</script>

<PageShell {title} {description}>
	{#if loading}
		<div class="certificate-grid" aria-label="กำลังโหลดคลังเกียรติบัตร" aria-busy="true">
			{#each { length: 3 }, index (index)}
				<div class="certificate-skeleton" aria-hidden="true"></div>
			{/each}
		</div>
	{:else if loadError}
		<PageState
			variant="error"
			title={loadError}
			description="ตรวจสอบการเชื่อมต่อแล้วลองโหลดอีกครั้ง"
			actionLabel="ลองใหม่"
			onaction={loadCertificates}
		/>
	{:else if certificates.length === 0}
		<PageState
			title="ยังไม่มีเกียรติบัตรในบัญชีนี้"
			description="เมื่อโรงเรียนออกเกียรติบัตรให้บัญชีนี้ รายการจะปรากฏที่นี่โดยอัตโนมัติ"
		/>
	{:else}
		<div class="registry-summary" aria-label="สรุปคลังเกียรติบัตร">
			<div>
				<span>ในทะเบียน</span>
				<strong>{certificates.length} ใบ</strong>
			</div>
			<p>ใช้ได้ {issuedCount} · เพิกถอน {revokedCount}</p>
			<Button variant="ghost" size="sm" onclick={loadCertificates}>
				<RefreshCw class="size-4" aria-hidden="true" /> โหลดใหม่
			</Button>
		</div>

		<div class="certificate-grid" data-testid="my-certificate-list">
			{#each certificates as certificate (certificate.id)}
				{@const downloadable =
					certificate.status === 'issued' && certificate.capabilities.canDownload === true}
				<article
					class={['certificate-docket', { revoked: certificate.status === 'revoked' }]}
					data-testid="my-certificate-card"
				>
					<div class="status-spine" aria-hidden="true"></div>
					<header>
						<div class="status-mark">
							{#if certificate.status === 'issued'}
								<ShieldCheck size={21} aria-hidden="true" />
								<span>ใช้ได้</span>
							{:else}
								<ShieldX size={21} aria-hidden="true" />
								<span>เพิกถอนแล้ว</span>
							{/if}
						</div>
						<div class="registry-number">
							<span>เลขทะเบียน</span>
							<strong>{certificate.certificateNumber}</strong>
						</div>
					</header>

					<div class="docket-body">
						<p class="template-name"><FileCheck2 size={16} /> {certificate.templateName}</p>
						<h2>{certificate.campaignName}</h2>
						{#if certificate.activityItem}
							<p class="activity"><Award size={16} /> {certificate.activityItem}</p>
						{/if}
						{#if certificate.awardOrRole}
							<p class="award-role">{certificate.awardOrRole}</p>
						{/if}
						<p class="issued-date">
							<CalendarDays size={16} />
							{formatThaiDate(certificate.issueDate)} · ปีการศึกษา
							{certificate.academicYearValue}
						</p>
					</div>

					<footer>
						<a
							class="verify-link"
							href={resolve(
								`/verify/certificate/${encodeURIComponent(certificate.certificateNumber)}` as '/verify/certificate/[certificateNumber]'
							)}
							target="_blank"
							rel="noopener noreferrer"
							referrerpolicy="no-referrer"
						>
							<ExternalLink size={16} aria-hidden="true" /> ตรวจสอบสาธารณะ
						</a>
						{#if downloadable}
							<Button
								size="sm"
								onclick={() => downloadCertificate(certificate)}
								disabled={downloadingId !== null}
								data-testid="my-certificate-download"
							>
								{#if downloadingId === certificate.id}
									<LoaderCircle class="size-4 animate-spin" aria-hidden="true" /> กำลังสร้าง PDF
								{:else}
									<Download class="size-4" aria-hidden="true" /> ดาวน์โหลด
								{/if}
							</Button>
						{/if}
					</footer>
				</article>
			{/each}
		</div>
	{/if}
</PageShell>

<style>
	.registry-summary {
		display: flex;
		align-items: center;
		gap: 1rem;
		margin-bottom: 1rem;
		padding: 0.8rem 1rem;
		border: 1px solid hsl(var(--border));
		background: linear-gradient(90deg, hsl(var(--muted) / 0.65), transparent);
	}

	.registry-summary div {
		display: flex;
		align-items: baseline;
		gap: 0.55rem;
	}

	.registry-summary span,
	.registry-summary p {
		color: hsl(var(--muted-foreground));
		font-size: 0.82rem;
	}

	.registry-summary p {
		margin: 0 auto 0 0;
	}

	.certificate-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 21rem), 1fr));
		gap: 1rem;
	}

	.certificate-skeleton {
		min-height: 20rem;
		border: 1px solid hsl(var(--border));
		background: linear-gradient(
			100deg,
			hsl(var(--muted) / 0.5) 30%,
			hsl(var(--muted)) 48%,
			hsl(var(--muted) / 0.5) 66%
		);
		background-size: 220% 100%;
		animation: shimmer 1.5s linear infinite;
	}

	.certificate-docket {
		--docket-accent: #176b58;
		position: relative;
		display: flex;
		min-width: 0;
		min-height: 20rem;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid hsl(var(--border));
		border-top: 3px solid #b88935;
		background: hsl(var(--card));
		box-shadow: 0 12px 30px rgb(23 50 77 / 0.07);
	}

	.certificate-docket.revoked {
		--docket-accent: #a03e49;
		border-top-color: #9d6870;
	}

	.status-spine {
		position: absolute;
		inset: 0 auto 0 0;
		width: 0.3rem;
		background: var(--docket-accent);
	}

	.certificate-docket header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 1rem;
		padding: 1.1rem 1.15rem 0.95rem 1.35rem;
		border-bottom: 1px dashed hsl(var(--border));
	}

	.status-mark {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		color: var(--docket-accent);
		font-size: 0.78rem;
		font-weight: 700;
	}

	.registry-number {
		min-width: 0;
		text-align: right;
	}

	.registry-number span {
		display: block;
		color: hsl(var(--muted-foreground));
		font-size: 0.68rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.registry-number strong {
		display: block;
		overflow-wrap: anywhere;
		color: #17324d;
		font-size: 0.92rem;
		font-variant-numeric: tabular-nums;
	}

	.docket-body {
		flex: 1;
		padding: 1.2rem 1.2rem 1.1rem 1.35rem;
	}

	.template-name,
	.activity,
	.issued-date {
		display: flex;
		align-items: center;
		gap: 0.45rem;
		margin: 0;
		color: hsl(var(--muted-foreground));
		font-size: 0.82rem;
	}

	.docket-body h2 {
		margin: 0.65rem 0 0.8rem;
		color: #17324d;
		font-size: 1.12rem;
		font-weight: 700;
		line-height: 1.45;
	}

	.activity {
		margin-bottom: 0.45rem;
	}

	.award-role {
		margin: 0 0 1rem;
		padding-left: 1.25rem;
		border-left: 2px solid #b88935;
		font-weight: 600;
	}

	.issued-date {
		margin-top: auto;
	}

	.certificate-docket footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		padding: 0.9rem 1.15rem 0.9rem 1.35rem;
		border-top: 1px solid hsl(var(--border));
		background: hsl(var(--muted) / 0.28);
	}

	.verify-link {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		color: hsl(var(--primary));
		font-size: 0.82rem;
		font-weight: 600;
		text-decoration: none;
	}

	.verify-link:hover {
		text-decoration: underline;
	}

	.verify-link:focus-visible {
		outline: 2px solid hsl(var(--ring));
		outline-offset: 3px;
	}

	@keyframes shimmer {
		to {
			background-position-x: -220%;
		}
	}

	@media (max-width: 520px) {
		.registry-summary,
		.certificate-docket footer {
			align-items: stretch;
			flex-direction: column;
		}

		.registry-summary p {
			margin-right: 0;
		}

		.certificate-docket header {
			flex-direction: column;
		}

		.registry-number {
			text-align: left;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.certificate-skeleton {
			animation: none;
		}
	}
</style>
