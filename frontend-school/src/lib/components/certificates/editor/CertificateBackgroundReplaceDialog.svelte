<script lang="ts">
	import type { CertificateRenderManifest, CertificateTemplateDetail } from '$lib/api/certificates';
	import CertificateBackgroundUpload from '$lib/components/certificates/CertificateBackgroundUpload.svelte';
	import * as Dialog from '$lib/components/ui/dialog';
	import { ArrowRight, Maximize2, RotateCcw, Scale } from 'lucide-svelte';
	import { toast } from 'svelte-sonner';

	let {
		open = $bindable(false),
		template,
		previewManifest,
		onmanifestrefresh,
		onpatched,
		onpendingchange
	}: {
		open?: boolean;
		template: CertificateTemplateDetail;
		previewManifest: CertificateRenderManifest;
		onmanifestrefresh: () => Promise<CertificateRenderManifest>;
		onpatched: (template: CertificateTemplateDetail) => void;
		onpendingchange: (pending: boolean) => void;
	} = $props();

	let pendingUpload = $state(false);

	function handleOpenChange(nextOpen: boolean) {
		if (!nextOpen && pendingUpload) {
			open = true;
			toast.error('แนบหรือลบไฟล์ชั่วคราวให้เสร็จก่อนปิดหน้าต่าง');
			return;
		}
		open = nextOpen;
	}

	function handlePendingChange(pending: boolean) {
		pendingUpload = pending;
		onpendingchange(pending);
	}

	function handlePatched(updated: CertificateTemplateDetail) {
		onpatched(updated);
		open = false;
	}
</script>

<Dialog.Root bind:open onOpenChange={handleOpenChange}>
	<Dialog.Content class="max-h-[92vh] overflow-y-auto sm:max-w-4xl">
		<Dialog.Header>
			<Dialog.Title>เปลี่ยน PDF พื้นหลัง</Dialog.Title>
			<Dialog.Description>
				พื้นหลังเป็นไฟล์ต้นฉบับของขนาดกระดาษ ตรวจตัวอย่างและเลือกผลต่อองค์ประกอบก่อนยืนยัน
			</Dialog.Description>
		</Dialog.Header>

		<div class="grid gap-3 md:grid-cols-3" aria-label="เปรียบเทียบวิธีจัดวางเมื่อขนาดเปลี่ยน">
			<div class="rounded-xl border bg-muted/15 p-3">
				<div class="flex items-center gap-2 text-xs font-semibold">
					<Maximize2 class="size-4 text-muted-foreground" /> ขนาดเดิมเท่ากัน
				</div>
				<p class="mt-1.5 text-[0.7rem] leading-relaxed text-muted-foreground">
					รักษาตำแหน่งและขนาดเดิมทุกจุด เหมาะกับการเปลี่ยนลายพื้นหลังเท่านั้น
				</p>
			</div>
			<div class="rounded-xl border border-blue-200 bg-blue-50/70 p-3">
				<div class="flex items-center gap-2 text-xs font-semibold text-blue-950">
					<Scale class="size-4" /> ปรับตามสัดส่วน
				</div>
				<p class="mt-1.5 text-[0.7rem] leading-relaxed text-blue-900">
					ย้ายและย่อขยายองค์ประกอบทั้งหมดตามกว้าง–สูงของหน้าใหม่ แล้วตรวจแก้อีกครั้ง
				</p>
			</div>
			<div class="rounded-xl border border-amber-200 bg-amber-50/70 p-3">
				<div class="flex items-center gap-2 text-xs font-semibold text-amber-950">
					<RotateCcw class="size-4" /> เริ่มจัดวางใหม่
				</div>
				<p class="mt-1.5 text-[0.7rem] leading-relaxed text-amber-900">
					ล้างองค์ประกอบเดิมเมื่อสัดส่วนต่างมาก ใช้พื้นหลังใหม่เป็นจุดเริ่มต้น
				</p>
			</div>
		</div>

		<div
			class="flex items-center gap-2 rounded-lg border border-dashed px-3 py-2 text-[0.7rem] text-muted-foreground"
		>
			<span>เลือกไฟล์</span><ArrowRight class="size-3.5" /><span>ดู PDF</span><ArrowRight
				class="size-3.5"
			/><span>เลือก scale/reset</span><ArrowRight class="size-3.5" /><span>ยืนยันแล้วจึงแนบ</span>
		</div>

		<CertificateBackgroundUpload
			{template}
			{previewManifest}
			{onmanifestrefresh}
			onpatched={handlePatched}
			onpendingchange={handlePendingChange}
		/>
	</Dialog.Content>
</Dialog.Root>
