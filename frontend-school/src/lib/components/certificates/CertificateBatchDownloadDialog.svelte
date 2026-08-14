<script lang="ts">
	import {
		createIssuedCertificateRenderManifests,
		type IssuedCertificateSummary
	} from '$lib/api/certificates';
	import {
		downloadCertificatePdf,
		MAX_CERTIFICATE_BATCH_SIZE,
		validateCertificateBatchSize
	} from '$lib/certificates/download';
	import { loadCertificateRenderer } from '$lib/certificates/renderer';
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { FileArchive, Files, ShieldCheck } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	let {
		open,
		campaignId,
		campaignName,
		certificates,
		selectedCertificateIds,
		onopenchange,
		ondownloaded
	}: {
		open: boolean;
		campaignId: string;
		campaignName: string;
		certificates: IssuedCertificateSummary[];
		selectedCertificateIds: string[];
		onopenchange: (open: boolean) => void;
		ondownloaded: () => void;
	} = $props();

	let busy = $state(false);
	let error = $state('');

	const selectedCertificates = $derived(
		selectedCertificateIds
			.map((id) => certificates.find((certificate) => certificate.id === id))
			.filter((certificate): certificate is IssuedCertificateSummary => certificate !== undefined)
	);

	function changeOpen(nextOpen: boolean) {
		if (busy) return;
		if (!nextOpen) error = '';
		onopenchange(nextOpen);
	}

	async function downloadBatch() {
		if (busy) return;
		error = '';
		try {
			validateCertificateBatchSize(selectedCertificateIds.length);
			if (
				selectedCertificates.length !== selectedCertificateIds.length ||
				selectedCertificates.some(
					(certificate) =>
						certificate.status !== 'issued' || certificate.capabilities.canDownload !== true
				)
			) {
				throw new Error('มีเกียรติบัตรที่เลือกอย่างน้อยหนึ่งใบซึ่งไม่มีสิทธิ์ดาวน์โหลด');
			}

			busy = true;
			const manifests = await createIssuedCertificateRenderManifests(campaignId, {
				certificateIds: selectedCertificateIds
			});
			if (manifests.length !== selectedCertificateIds.length) {
				throw new Error('จำนวนไฟล์ที่เตรียมได้ไม่ตรงกับรายการที่เลือก');
			}
			const renderer = await loadCertificateRenderer();
			const bytes = await renderer.buildCertificatePdf(manifests);
			downloadCertificatePdf(
				bytes,
				`เกียรติบัตร-${campaignName}-${selectedCertificateIds.length.toLocaleString('th-TH')}-ใบ.pdf`
			);
			toast.success(
				`สร้าง PDF รวม ${selectedCertificateIds.length.toLocaleString('th-TH')} ใบแล้ว`
			);
			error = '';
			ondownloaded();
			onopenchange(false);
		} catch (downloadError) {
			error = downloadError instanceof Error ? downloadError.message : 'สร้าง PDF รวมไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}
</script>

<Dialog.Root {open} onOpenChange={changeOpen}>
	<Dialog.Content class="sm:max-w-lg">
		<Dialog.Header>
			<Dialog.Title
				>ดาวน์โหลดรวม {selectedCertificateIds.length.toLocaleString('th-TH')} ใบ</Dialog.Title
			>
			<Dialog.Description>
				ระบบจะสร้าง PDF ใหม่จากแบบปัจจุบัน และเรียงหน้าตามลำดับที่คุณเลือก
			</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4">
			<div class="grid grid-cols-[auto_1fr] gap-3 rounded-xl border bg-muted/30 p-4">
				<div class="flex size-10 items-center justify-center rounded-lg bg-primary/10 text-primary">
					<FileArchive class="size-5" />
				</div>
				<div>
					<p class="font-medium">หนึ่งไฟล์ หลายขนาดกระดาษ</p>
					<p class="mt-1 text-sm text-muted-foreground">
						แต่ละหน้ารักษาขนาดและแนวกระดาษของแบบนั้นไว้ รองรับสูงสุด
						{MAX_CERTIFICATE_BATCH_SIZE.toLocaleString('th-TH')} ใบต่อครั้ง
					</p>
				</div>
			</div>

			<div class="flex items-start gap-2 rounded-lg bg-blue-50 px-4 py-3 text-xs text-blue-900">
				<ShieldCheck class="mt-0.5 size-4 shrink-0" />
				<span>การสร้างไฟล์ไม่เปลี่ยนสถานะหรือเลขเกียรติบัตร หากสร้างไม่สำเร็จสามารถลองใหม่ได้</span>
			</div>

			{#if error}
				<div
					role="alert"
					class="rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm text-destructive"
				>
					{error}
				</div>
			{/if}
		</div>

		<Dialog.Footer>
			<Button variant="outline" disabled={busy} onclick={() => changeOpen(false)}>ยกเลิก</Button>
			<LoadingButton
				loading={busy}
				loadingLabel="กำลังสร้าง PDF..."
				disabled={selectedCertificateIds.length === 0}
				onclick={downloadBatch}
			>
				<Files class="size-4" /> สร้าง PDF รวม
			</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
