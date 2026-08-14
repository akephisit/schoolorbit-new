<script lang="ts">
	import {
		createIssuedCertificateRenderManifest,
		type IssuedCertificateSummary
	} from '$lib/api/certificates';
	import { downloadCertificatePdf } from '$lib/certificates/download';
	import { loadCertificateRenderer } from '$lib/certificates/renderer';
	import { LoadingButton } from '$lib/components/app-state';
	import { Download } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	let {
		certificate,
		canDownload = false
	}: {
		certificate: IssuedCertificateSummary;
		canDownload?: boolean;
	} = $props();

	let busy = $state(false);
	const downloadable = $derived(
		canDownload && certificate.status === 'issued' && certificate.capabilities.canDownload === true
	);

	async function download() {
		if (!downloadable || busy) return;
		busy = true;
		try {
			const manifest = await createIssuedCertificateRenderManifest(certificate.id);
			const renderer = await loadCertificateRenderer();
			const bytes = await renderer.buildCertificatePdf([manifest]);
			downloadCertificatePdf(bytes, manifest.suggestedFilename);
			toast.success(`ดาวน์โหลด ${certificate.certificateNumber} แล้ว`);
		} catch (downloadError) {
			toast.error(
				downloadError instanceof Error ? downloadError.message : 'สร้างไฟล์เกียรติบัตรไม่สำเร็จ'
			);
		} finally {
			busy = false;
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
