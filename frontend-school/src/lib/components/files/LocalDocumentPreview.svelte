<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import type { PdfPreview } from '#lib/files/document-preview.js';
	let { file }: { file: File } = $props();
	let opened = $state(false),
		loading = $state(false),
		error = $state('');
	let imageUrl = $state(''),
		pdf = $state<PdfPreview | null>(null);
	let canvas = $state<HTMLCanvasElement>(),
		page = $state(1);
	const supported = $derived(file.type === 'application/pdf' || file.type.startsWith('image/'));
	$effect(() => {
		const source = file;
		if (!opened || !supported) return;
		const controller = new AbortController();
		let document: PdfPreview | undefined;
		let objectUrl: string | undefined;
		loading = true;
		error = '';
		page = 1;
		if (source.type.startsWith('image/')) {
			objectUrl = URL.createObjectURL(source);
			imageUrl = objectUrl;
			loading = false;
		} else {
			void (async () => {
				try {
					const { loadPdfPreview } = await import('#lib/files/document-preview.js');
					document = await loadPdfPreview(source, controller.signal);
					if (controller.signal.aborted) {
						await document.dispose();
						return;
					}
					pdf = document;
				} catch (cause) {
					if (!controller.signal.aborted)
						error = cause instanceof Error ? cause.message : 'เปิดตัวอย่างไฟล์ไม่สำเร็จ';
				} finally {
					if (!controller.signal.aborted) loading = false;
				}
			})();
		}
		return () => {
			controller.abort();
			if (objectUrl) URL.revokeObjectURL(objectUrl);
			if (document) void document.dispose();
			imageUrl = '';
			pdf = null;
		};
	});
	$effect(() => {
		const document = pdf,
			target = canvas,
			number = page;
		if (!document || !target) return;
		const controller = new AbortController();
		void document.renderPage(number, target, controller.signal).catch((cause) => {
			if (!controller.signal.aborted)
				error = cause instanceof Error ? cause.message : 'แสดงหน้าตัวอย่างไม่สำเร็จ';
		});
		return () => controller.abort();
	});
</script>

<section class="min-w-0 space-y-2 rounded-md border p-3" aria-label={`ตัวอย่าง ${file.name}`}>
	<div class="flex flex-wrap items-center justify-between gap-2">
		<p class="min-w-0 break-words text-sm">{file.name}</p>
		{#if supported}<Button
				type="button"
				variant="outline"
				size="sm"
				onclick={() => (opened = !opened)}
			>
				{opened ? 'ปิดตัวอย่าง' : 'ดูตัวอย่างก่อนบันทึก'}
			</Button>{/if}
	</div>
	{#if opened && supported}
		{#if loading}<p role="status" class="text-sm text-muted-foreground">กำลังเปิดตัวอย่าง…</p>{/if}
		{#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
		{#if imageUrl}<img
				src={imageUrl}
				alt={`ตัวอย่าง ${file.name}`}
				class="max-h-96 max-w-full object-contain"
			/>{/if}
		{#if pdf}
			<div class="flex flex-wrap items-center gap-2">
				<Button
					type="button"
					size="sm"
					variant="outline"
					disabled={page <= 1}
					onclick={() => page--}>หน้าก่อน</Button
				>
				<span class="text-sm">หน้า {page} / {pdf.pageCount}</span>
				<Button
					type="button"
					size="sm"
					variant="outline"
					disabled={page >= pdf.pageCount}
					onclick={() => page++}>หน้าถัดไป</Button
				>
			</div>
			<canvas bind:this={canvas} class="max-w-full" aria-label={`ตัวอย่าง PDF ${file.name}`}
			></canvas>
		{/if}
	{/if}
</section>
