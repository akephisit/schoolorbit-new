<script lang="ts">
	import { onDestroy } from 'svelte';
	import { LatestRequest } from '$lib/async/latest-request';
	import {
		createIssuedCertificateRenderManifest,
		type IssuedCertificateSummary
	} from '$lib/api/certificates';
	import { downloadCertificatePdf } from '$lib/certificates/download';
	import { loadCertificateRenderer } from '$lib/certificates/renderer';
	import { LoadingButton } from '$lib/components/app-state';
	import { Download } from '@lucide/svelte';
	import { toast } from 'svelte-sonner';

	let {
		certificate,
		canDownload = false
	}: {
		certificate: IssuedCertificateSummary;
		canDownload?: boolean;
	} = $props();

	let busy = $state(false);
	let disposed = false;
	const request = new LatestRequest();
	onDestroy(() => {
		disposed = true;
		request.abort();
	});
	const downloadable = $derived(
		canDownload && certificate.status === 'issued' && certificate.capabilities.canDownload === true
	);

	async function download() {
		if (disposed || !downloadable || busy) return;
		const selectedId = certificate.id,
			t = request.begin();
		const current = () =>
			!disposed && request.isCurrent(t.revision) && selectedId === certificate.id && downloadable;
		busy = true;
		try {
			const manifest = await createIssuedCertificateRenderManifest(selectedId, {
				signal: t.signal
			});
			if (!current()) return;
			const renderer = await loadCertificateRenderer();
			if (!current()) return;
			const bytes = await renderer.buildCertificatePdf([manifest]);
			if (!current()) return;
			downloadCertificatePdf(bytes, manifest.suggestedFilename);
			toast.success(`ดาวน์โหลด ${certificate.certificateNumber} แล้ว`);
		} catch (downloadError) {
			if (!current()) return;
			toast.error(
				downloadError instanceof Error ? downloadError.message : 'สร้างไฟล์เกียรติบัตรไม่สำเร็จ'
			);
		} finally {
			if (!disposed) busy = false;
		}
	}
</script>

{#if downloadable}
	<LoadingButton
		loading={busy}
		loadingLabel="กำลังสร้าง..."
		size="sm"
		variant="outline"
		onclick={download}
		aria-label={`ดาวน์โหลด ${certificate.certificateNumber}`}
	>
		<Download class="size-4" /> ดาวน์โหลด
	</LoadingButton>
{/if}
