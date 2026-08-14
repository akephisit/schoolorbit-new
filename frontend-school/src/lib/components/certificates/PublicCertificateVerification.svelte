<script lang="ts">
	import {
		createPublicCertificateRenderManifest,
		verifyCertificateByQr,
		verifyCertificateManually,
		type PublicCertificateVerificationData
	} from '$lib/api/public-certificates';
	import { downloadCertificatePdf } from '$lib/certificates/download';
	import { loadCertificateRenderer } from '$lib/certificates/renderer';
	import {
		Award,
		Building2,
		CalendarDays,
		Download,
		FileCheck2,
		LoaderCircle,
		ScanLine,
		Search,
		ShieldCheck,
		ShieldX
	} from 'lucide-svelte';
	import { onMount } from 'svelte';

	let {
		initialNumber = '',
		autoVerifyQr = false
	}: {
		initialNumber?: string;
		autoVerifyQr?: boolean;
	} = $props();

	const genericFailure = 'ไม่พบข้อมูลที่ตรงกัน';
	let certificateNumber = $state('');
	let firstName = $state('');
	let lastName = $state('');
	let result = $state.raw<PublicCertificateVerificationData | null>(null);
	let verificationError = $state('');
	let downloadError = $state('');
	let verifying = $state(false);
	let downloading = $state(false);
	let requestSequence = 0;
	let verificationController: AbortController | null = null;
	let downloadController: AbortController | null = null;

	const canDownload = $derived(
		result?.status === 'issued' && typeof result.receipt === 'string' && result.receipt.length > 0
	);
	const recipientName = $derived(
		result ? `${result.title ?? ''}${result.firstName} ${result.lastName}`.trim() : ''
	);

	onMount(() => {
		certificateNumber = initialNumber;
		if (autoVerifyQr) {
			const fragment = window.location.hash;
			const fragmentValues = new URLSearchParams(fragment.startsWith('#') ? fragment.slice(1) : '');
			const proofValues = fragmentValues.getAll('proof');
			const proof = proofValues.length === 1 ? (proofValues[0] ?? '') : '';
			window.history.replaceState(
				window.history.state,
				'',
				`${window.location.pathname}${window.location.search}`
			);
			if (proof && certificateNumber.trim()) {
				void verifyQrProof(proof);
			} else {
				verificationError = genericFailure;
			}
		}

		return () => {
			verificationController?.abort();
			downloadController?.abort();
		};
	});

	async function runVerification(
		request: (signal: AbortSignal) => Promise<PublicCertificateVerificationData>
	): Promise<void> {
		const sequence = ++requestSequence;
		verificationController?.abort();
		const controller = new AbortController();
		verificationController = controller;
		verifying = true;
		verificationError = '';
		downloadError = '';
		result = null;

		try {
			const verified = await request(controller.signal);
			if (sequence !== requestSequence || controller.signal.aborted) return;
			result = verified;
		} catch {
			if (sequence !== requestSequence || controller.signal.aborted) return;
			verificationError = genericFailure;
		} finally {
			if (sequence === requestSequence) verifying = false;
		}
	}

	async function verifyQrProof(proof: string): Promise<void> {
		await runVerification((signal) =>
			verifyCertificateByQr(
				{
					certificateNumber: certificateNumber.trim(),
					proof
				},
				{ signal }
			)
		);
	}

	async function submitManualVerification(event: SubmitEvent): Promise<void> {
		event.preventDefault();
		if (!certificateNumber.trim() || !firstName.trim() || !lastName.trim()) {
			result = null;
			verificationError = 'กรอกเลขเกียรติบัตร ชื่อ และนามสกุลให้ครบ';
			return;
		}

		await runVerification((signal) =>
			verifyCertificateManually(
				{
					certificateNumber: certificateNumber.trim(),
					firstName: firstName.trim(),
					lastName: lastName.trim()
				},
				{ signal }
			)
		);
	}

	async function downloadCertificate(): Promise<void> {
		if (!result || !canDownload || !result.receipt || downloading) return;
		downloadController?.abort();
		const controller = new AbortController();
		downloadController = controller;
		downloading = true;
		downloadError = '';
		try {
			const manifest = await createPublicCertificateRenderManifest(
				{ receipt: result.receipt },
				{ signal: controller.signal }
			);
			controller.signal.throwIfAborted();
			const renderer = await loadCertificateRenderer();
			const bytes = await renderer.buildCertificatePdf([manifest]);
			controller.signal.throwIfAborted();
			downloadCertificatePdf(bytes, manifest.suggestedFilename);
		} catch {
			if (controller.signal.aborted) return;
			downloadError = 'สร้างไฟล์ไม่สำเร็จ กรุณาตรวจสอบเกียรติบัตรใหม่แล้วลองอีกครั้ง';
		} finally {
			if (downloadController === controller) downloading = false;
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

<main class="verification-page">
	<div class="ambient-grid" aria-hidden="true"></div>
	<section class="verification-shell" aria-labelledby="verification-title">
		<header class="page-heading">
			<div class="brand-mark" aria-hidden="true"><ShieldCheck size={30} strokeWidth={1.7} /></div>
			<div>
				<p class="eyebrow">SchoolOrbit · Certificate registry</p>
				<h1 id="verification-title">ตรวจสอบเกียรติบัตร</h1>
				<p class="heading-copy">ยืนยันข้อมูลจากเลขบนเกียรติบัตร หรือเปิดหน้านี้ผ่าน QR Code</p>
			</div>
		</header>

		<div class="registry-rail" aria-label="เลขเกียรติบัตรที่กำลังตรวจสอบ">
			<span>เลขทะเบียน</span>
			<strong>{certificateNumber.trim() || '2569-0000-000000-0'}</strong>
		</div>

		<div class="workspace">
			<section class="manual-panel" aria-labelledby="manual-title">
				<div class="section-heading">
					<Search size={20} aria-hidden="true" />
					<div>
						<h2 id="manual-title">กรอกข้อมูลเพื่อตรวจสอบ</h2>
						<p>กรอกชื่อและนามสกุลแยกช่องให้ตรงกับเกียรติบัตร</p>
					</div>
				</div>

				<form data-testid="certificate-verification-form" onsubmit={submitManualVerification}>
					<label for="certificate-number">เลขเกียรติบัตร</label>
					<input
						id="certificate-number"
						name="certificateNumber"
						autocomplete="off"
						placeholder="เช่น 2569-0042-000123-4"
						bind:value={certificateNumber}
					/>

					<div class="name-fields">
						<div>
							<label for="first-name">ชื่อ</label>
							<input
								id="first-name"
								name="firstName"
								autocomplete="given-name"
								bind:value={firstName}
							/>
						</div>
						<div>
							<label for="last-name">นามสกุล</label>
							<input
								id="last-name"
								name="lastName"
								autocomplete="family-name"
								bind:value={lastName}
							/>
						</div>
					</div>

					<button class="verify-button" type="submit" disabled={verifying}>
						{#if verifying}
							<LoaderCircle class="spin" size={18} aria-hidden="true" /> กำลังตรวจสอบ
						{:else}
							<Search size={18} aria-hidden="true" /> ตรวจสอบข้อมูล
						{/if}
					</button>
				</form>

				<div class="privacy-note">
					<ScanLine size={18} aria-hidden="true" />
					<span
						>ข้อมูลใช้เพื่อตรวจสอบครั้งนี้เท่านั้น ระบบไม่แสดงเหตุผลภายในหรือข้อมูลบัญชีผู้รับ</span
					>
				</div>
			</section>

			<section class="result-panel" aria-live="polite" aria-busy={verifying}>
				{#if verifying}
					<div class="result-placeholder">
						<LoaderCircle class="spin" size={34} aria-hidden="true" />
						<h2>กำลังตรวจสอบทะเบียน</h2>
						<p>โปรดรอสักครู่</p>
					</div>
				{:else if verificationError}
					<div class="result-placeholder error-state" data-testid="verification-error">
						<ShieldX size={38} aria-hidden="true" />
						<h2>ตรวจสอบไม่สำเร็จ</h2>
						<p>{verificationError}</p>
						<small>ตรวจเลข ชื่อ และนามสกุลอีกครั้ง หรือสแกน QR Code ใหม่</small>
					</div>
				{:else if result}
					<div class="certificate-result" data-testid="verification-result">
						<div class:revoked={result.status === 'revoked'} class="status-seal">
							{#if result.status === 'issued'}
								<ShieldCheck size={31} aria-hidden="true" />
								<div><span>สถานะ</span><strong>ใช้ได้</strong></div>
							{:else}
								<ShieldX size={31} aria-hidden="true" />
								<div><span>สถานะ</span><strong>เพิกถอนแล้ว</strong></div>
							{/if}
						</div>

						<div class="certificate-number">
							<span>เลขเกียรติบัตร</span>
							<strong>{result.certificateNumber}</strong>
						</div>

						<div class="recipient">
							<span>มอบให้</span>
							<h2>{recipientName}</h2>
						</div>

						<dl class="certificate-details">
							<div>
								<dt><Award size={17} aria-hidden="true" /> กิจกรรม</dt>
								<dd>{result.campaignName}</dd>
							</div>
							<div>
								<dt><FileCheck2 size={17} aria-hidden="true" /> แบบเกียรติบัตร</dt>
								<dd>{result.templateName}</dd>
							</div>
							{#if result.activityItem}
								<div>
									<dt>รายการ</dt>
									<dd>{result.activityItem}</dd>
								</div>
							{/if}
							{#if result.awardOrRole}
								<div>
									<dt>รางวัลหรือบทบาท</dt>
									<dd>{result.awardOrRole}</dd>
								</div>
							{/if}
							<div>
								<dt><CalendarDays size={17} aria-hidden="true" /> วันที่ออก</dt>
								<dd>{formatThaiDate(result.issueDate)} · ปีการศึกษา {result.academicYear}</dd>
							</div>
							<div>
								<dt><Building2 size={17} aria-hidden="true" /> ผู้ออก</dt>
								<dd>{result.issuerSchoolName}</dd>
							</div>
						</dl>

						{#if result.status === 'issued' && result.receipt}
							<button
								class="download-button"
								type="button"
								onclick={downloadCertificate}
								disabled={downloading}
								data-testid="public-certificate-download"
							>
								{#if downloading}
									<LoaderCircle class="spin" size={18} aria-hidden="true" /> กำลังสร้าง PDF
								{:else}
									<Download size={18} aria-hidden="true" /> ดาวน์โหลดเกียรติบัตร
								{/if}
							</button>
						{:else if result.status === 'revoked'}
							<div class="revoked-note">
								<p>เกียรติบัตรฉบับนี้ถูกเพิกถอนและไม่สามารถดาวน์โหลดได้</p>
								{#if result.replacementCertificateNumber}
									<span>เลขใบทดแทน: <strong>{result.replacementCertificateNumber}</strong></span>
								{/if}
							</div>
						{/if}

						{#if downloadError}<p class="download-error">{downloadError}</p>{/if}
					</div>
				{:else}
					<div class="result-placeholder">
						<div class="document-icon" aria-hidden="true"><FileCheck2 size={34} /></div>
						<h2>ผลการตรวจสอบจะแสดงที่นี่</h2>
						<p>ระบบแสดงเฉพาะข้อมูลสาธารณะที่จำเป็นบนเกียรติบัตร</p>
					</div>
				{/if}
			</section>
		</div>
	</section>
</main>

<style>
	.verification-page {
		--registry-ink: #17324d;
		--registry-blue: #2d648c;
		--registry-mist: #eaf2f7;
		--registry-line: #c8d8e4;
		--registry-gold: #b9872e;
		--verified: #167055;
		--revoked: #a23d46;
		position: relative;
		isolation: isolate;
		min-height: 100svh;
		overflow: hidden;
		background: #f4f8fb;
		color: var(--registry-ink);
		padding: clamp(1.25rem, 4vw, 4rem) 1rem;
	}

	.ambient-grid {
		position: absolute;
		z-index: -1;
		inset: 0;
		background-image:
			linear-gradient(rgb(45 100 140 / 0.055) 1px, transparent 1px),
			linear-gradient(90deg, rgb(45 100 140 / 0.055) 1px, transparent 1px);
		background-size: 32px 32px;
		mask-image: linear-gradient(to bottom, #000, transparent 78%);
	}

	.verification-shell {
		width: min(1120px, 100%);
		margin: 0 auto;
		background: rgb(255 255 255 / 0.97);
		border: 1px solid var(--registry-line);
		box-shadow: 0 24px 70px rgb(23 50 77 / 0.12);
	}

	.page-heading {
		display: flex;
		align-items: center;
		gap: 1rem;
		padding: clamp(1.4rem, 3vw, 2.4rem);
	}

	.brand-mark {
		display: grid;
		place-items: center;
		width: 3.5rem;
		aspect-ratio: 1;
		border: 1px solid var(--registry-gold);
		color: var(--registry-gold);
		transform: rotate(45deg);
	}

	.brand-mark :global(svg) {
		transform: rotate(-45deg);
	}

	.eyebrow {
		margin: 0 0 0.25rem;
		color: var(--registry-blue);
		font-size: 0.72rem;
		font-weight: 600;
		letter-spacing: 0.13em;
		text-transform: uppercase;
	}

	h1,
	h2,
	p {
		margin-top: 0;
	}

	h1 {
		margin-bottom: 0.25rem;
		font-size: clamp(1.8rem, 4vw, 3rem);
		font-weight: 600;
		line-height: 1.1;
		letter-spacing: -0.035em;
	}

	.heading-copy {
		margin-bottom: 0;
		color: #5a7083;
	}

	.registry-rail {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 1rem;
		min-height: 4.6rem;
		padding: 1rem clamp(1.4rem, 3vw, 2.4rem);
		border-block: 1px solid var(--registry-line);
		background: var(--registry-ink);
		color: white;
	}

	.registry-rail span,
	.certificate-number span,
	.recipient > span {
		font-size: 0.74rem;
		font-weight: 500;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		opacity: 0.72;
	}

	.registry-rail strong,
	.certificate-number strong {
		font-family: 'IBM Plex Mono', 'Noto Sans Mono', monospace;
		font-size: clamp(1rem, 2.4vw, 1.4rem);
		letter-spacing: 0.045em;
		font-variant-numeric: tabular-nums;
	}

	.workspace {
		display: grid;
		grid-template-columns: minmax(0, 0.9fr) minmax(0, 1.1fr);
	}

	.manual-panel,
	.result-panel {
		min-width: 0;
		padding: clamp(1.4rem, 3vw, 2.4rem);
	}

	.manual-panel {
		border-right: 1px solid var(--registry-line);
		background: #fbfdfe;
	}

	.section-heading {
		display: flex;
		gap: 0.75rem;
		align-items: flex-start;
		margin-bottom: 1.4rem;
	}

	.section-heading > :global(svg) {
		flex: 0 0 auto;
		margin-top: 0.2rem;
		color: var(--registry-blue);
	}

	.section-heading h2,
	.result-placeholder h2 {
		margin-bottom: 0.2rem;
		font-size: 1.08rem;
		font-weight: 600;
	}

	.section-heading p,
	.result-placeholder p {
		margin-bottom: 0;
		color: #65798a;
		font-size: 0.88rem;
		line-height: 1.6;
	}

	form {
		display: grid;
		gap: 0.75rem;
	}

	label {
		font-size: 0.82rem;
		font-weight: 550;
	}

	input {
		width: 100%;
		height: 2.85rem;
		margin-top: 0.35rem;
		border: 1px solid #b8cbd8;
		border-radius: 0.35rem;
		background: white;
		padding: 0 0.8rem;
		color: var(--registry-ink);
		font: inherit;
		transition:
			border-color 150ms ease,
			box-shadow 150ms ease;
	}

	input:focus-visible {
		outline: none;
		border-color: var(--registry-blue);
		box-shadow: 0 0 0 3px rgb(45 100 140 / 0.15);
	}

	.name-fields {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.75rem;
	}

	.verify-button,
	.download-button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		min-height: 2.85rem;
		border: 0;
		border-radius: 0.35rem;
		font: inherit;
		font-weight: 550;
		cursor: pointer;
		transition:
			transform 150ms ease,
			background 150ms ease;
	}

	.verify-button {
		margin-top: 0.45rem;
		background: var(--registry-ink);
		color: white;
	}

	.verify-button:hover:not(:disabled),
	.download-button:hover:not(:disabled) {
		transform: translateY(-1px);
	}

	button:focus-visible {
		outline: 3px solid rgb(45 100 140 / 0.25);
		outline-offset: 2px;
	}

	button:disabled {
		cursor: wait;
		opacity: 0.66;
	}

	.privacy-note {
		display: flex;
		gap: 0.6rem;
		margin-top: 1.2rem;
		padding-top: 1rem;
		border-top: 1px dashed var(--registry-line);
		color: #65798a;
		font-size: 0.77rem;
		line-height: 1.55;
	}

	.privacy-note > :global(svg) {
		flex: 0 0 auto;
	}

	.result-panel {
		display: grid;
		min-height: 31rem;
	}

	.result-placeholder {
		place-self: center;
		max-width: 22rem;
		text-align: center;
	}

	.result-placeholder > :global(svg),
	.document-icon {
		margin: 0 auto 0.9rem;
		color: var(--registry-blue);
	}

	.document-icon {
		display: grid;
		place-items: center;
		width: 4.5rem;
		aspect-ratio: 1;
		border: 1px dashed var(--registry-line);
	}

	.error-state > :global(svg) {
		color: var(--revoked);
	}

	.error-state small {
		display: block;
		margin-top: 0.75rem;
		color: #7a8995;
	}

	.certificate-result {
		display: flex;
		flex-direction: column;
		gap: 1.1rem;
	}

	.status-seal {
		display: inline-flex;
		align-self: flex-start;
		align-items: center;
		gap: 0.65rem;
		border: 1px solid currentColor;
		padding: 0.55rem 0.8rem;
		color: var(--verified);
	}

	.status-seal.revoked {
		color: var(--revoked);
	}

	.status-seal span,
	.status-seal strong {
		display: block;
		line-height: 1.1;
	}

	.status-seal span {
		font-size: 0.67rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.status-seal strong {
		margin-top: 0.2rem;
		font-size: 0.95rem;
	}

	.certificate-number {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 1rem;
		padding-bottom: 0.9rem;
		border-bottom: 2px solid var(--registry-gold);
	}

	.recipient h2 {
		margin: 0.25rem 0 0;
		font-family: 'Sarabun', 'Noto Sans Thai', Tahoma, sans-serif;
		font-size: clamp(1.65rem, 3vw, 2.35rem);
		font-weight: 600;
		line-height: 1.25;
	}

	.certificate-details {
		display: grid;
		gap: 0;
		margin: 0;
		border-top: 1px solid var(--registry-line);
	}

	.certificate-details > div {
		display: grid;
		grid-template-columns: minmax(7.5rem, 0.42fr) 1fr;
		gap: 1rem;
		padding: 0.72rem 0;
		border-bottom: 1px solid var(--registry-line);
	}

	dt {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		color: #65798a;
		font-size: 0.78rem;
	}

	dd {
		margin: 0;
		font-size: 0.9rem;
		font-weight: 520;
	}

	.download-button {
		width: 100%;
		background: var(--verified);
		color: white;
	}

	.revoked-note {
		border-left: 3px solid var(--revoked);
		background: #fff5f5;
		padding: 0.9rem 1rem;
		color: #7f3139;
	}

	.revoked-note p {
		margin-bottom: 0.3rem;
	}

	.revoked-note span {
		font-size: 0.82rem;
	}

	.download-error {
		margin: -0.35rem 0 0;
		color: var(--revoked);
		font-size: 0.82rem;
		text-align: center;
	}

	:global(.spin) {
		animation: spin 0.9s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	@media (max-width: 760px) {
		.verification-page {
			padding: 0;
		}

		.verification-shell {
			min-height: 100svh;
			border-inline: 0;
			box-shadow: none;
		}

		.workspace {
			grid-template-columns: 1fr;
		}

		.manual-panel {
			border-right: 0;
			border-bottom: 1px solid var(--registry-line);
		}

		.result-panel {
			min-height: 25rem;
		}
	}

	@media (max-width: 470px) {
		.page-heading {
			align-items: flex-start;
		}

		.brand-mark {
			width: 2.8rem;
		}

		.registry-rail,
		.certificate-number {
			align-items: flex-start;
			flex-direction: column;
			gap: 0.35rem;
		}

		.name-fields,
		.certificate-details > div {
			grid-template-columns: 1fr;
			gap: 0.25rem;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		*,
		*::before,
		*::after {
			scroll-behavior: auto !important;
			transition-duration: 0.01ms !important;
			animation-duration: 0.01ms !important;
			animation-iteration-count: 1 !important;
		}
	}
</style>
