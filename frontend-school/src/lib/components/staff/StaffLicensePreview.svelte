<script lang="ts">
	import { MediaQuery } from 'svelte/reactivity';
	import { Download, ChevronLeft, ChevronRight, FileSearch } from '@lucide/svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { downloadFile, getFileMetadata } from '#lib/api/files.js';
	import { loadPdfPreview, type PdfPreview } from '#lib/files/document-preview.js';
	let {
		fileId,
		staffId,
		disabled = false
	}: { fileId: string; staffId: string; disabled?: boolean } = $props();
	let open = $state(false),
		loading = $state(false),
		error = $state(''),
		name = $state('ใบประกอบวิชาชีพ');
	let url = $state(''),
		pdf = $state.raw<PdfPreview | null>(null),
		firstPage = $state(1);
	let firstCanvas = $state<HTMLCanvasElement>(),
		secondCanvas = $state<HTMLCanvasElement>();
	const wide = new MediaQuery('(min-width: 768px)');
	const step = $derived(wide.current ? 2 : 1);
	let rendering = $state(false),
		renderError = $state('');
	$effect(() => {
		const resource = staffId,
			file = fileId;
		if (!open || disabled) return;
		const controller = new AbortController();
		let objectUrl = '',
			document: PdfPreview | null = null;
		loading = true;
		error = '';
		url = '';
		pdf = null;
		firstPage = 1;
		void (async () => {
			try {
				const [metadata, blob] = await Promise.all([
					getFileMetadata(file, resource),
					downloadFile(file, resource, controller.signal)
				]);
				controller.signal.throwIfAborted();
				name = metadata.displayFilename;
				if (blob.type === 'application/pdf') {
					document = await loadPdfPreview(blob, controller.signal);
					controller.signal.throwIfAborted();
					pdf = document;
				} else if (['image/jpeg', 'image/png', 'image/webp'].includes(blob.type)) {
					objectUrl = URL.createObjectURL(blob);
				} else throw new Error('ไม่รองรับเอกสารชนิดนี้');
				if (!objectUrl) objectUrl = URL.createObjectURL(blob);
				url = objectUrl;
			} catch (cause) {
				if (!controller.signal.aborted)
					error = cause instanceof Error ? cause.message : 'เปิดเอกสารไม่สำเร็จ';
			} finally {
				if (!controller.signal.aborted) loading = false;
			}
		})();
		return () => {
			controller.abort();
			if (objectUrl) URL.revokeObjectURL(objectUrl);
			if (document) void document.dispose();
		};
	});
	$effect(() => {
		const document = pdf,
			page = firstPage,
			count = step,
			first = firstCanvas,
			second = secondCanvas;
		if (!open || !document || !first) return;
		const controller = new AbortController();
		rendering = true;
		renderError = '';
		void Promise.all([
			document.renderPage(page, first, controller.signal),
			...(count === 2 && second && page < document.pageCount
				? [document.renderPage(page + 1, second, controller.signal)]
				: [])
		])
			.catch((cause: unknown) => {
				if (!controller.signal.aborted)
					renderError = cause instanceof Error ? cause.message : 'แสดงหน้าเอกสารไม่สำเร็จ';
			})
			.finally(() => {
				if (!controller.signal.aborted) rendering = false;
			});
		return () => controller.abort();
	});
</script>

<Button type="button" variant="outline" size="sm" {disabled} onclick={() => (open = true)}
	><FileSearch class="size-4" />ดูใบประกอบวิชาชีพ</Button
>
<Dialog.Root bind:open>
	<Dialog.Content class="flex max-h-[90dvh] flex-col gap-3 overflow-hidden p-4 sm:max-w-5xl">
		<Dialog.Header class="pr-8"
			><Dialog.Title>ใบประกอบวิชาชีพ</Dialog.Title><Dialog.Description class="break-all"
				>{name}</Dialog.Description
			></Dialog.Header
		>
		{#if loading}<p role="status" class="py-8 text-center text-sm">กำลังเปิดเอกสาร…</p>
		{:else if error}<p role="alert" class="text-sm text-destructive">{error}</p>
			<Button
				variant="outline"
				onclick={() => {
					open = false;
				}}>ปิด</Button
			>
		{:else if url}
			<div class="min-h-0 overflow-auto rounded-lg bg-muted/40 p-2" aria-busy={rendering}>
				{#if pdf}
					<div class="grid items-start gap-3 md:grid-cols-2" data-testid="licence-pdf-pages">
						<canvas
							bind:this={firstCanvas}
							class="h-auto w-full bg-white"
							aria-label={`เอกสารหน้า ${firstPage}`}
						></canvas>
						{#if step === 2 && firstPage < pdf.pageCount}<canvas
								bind:this={secondCanvas}
								class="h-auto w-full bg-white"
								aria-label={`เอกสารหน้า ${firstPage + 1}`}
							></canvas>{/if}
					</div>
				{:else}<img
						src={url}
						alt="ใบประกอบวิชาชีพที่แนบ"
						class="mx-auto h-auto max-h-[65dvh] max-w-full object-contain"
					/>{/if}
			</div>
			{#if renderError}<p role="alert" class="text-sm text-destructive">{renderError}</p>{/if}
			<div class="flex flex-wrap items-center justify-between gap-2">
				{#if pdf}<div class="flex items-center gap-2">
						<Button
							type="button"
							variant="outline"
							size="icon-sm"
							aria-label="หน้าก่อนหน้า"
							disabled={firstPage === 1 || rendering}
							onclick={() => (firstPage = Math.max(1, firstPage - step))}
							><ChevronLeft class="size-4" /></Button
						>
						<p class="text-xs" aria-live="polite">
							หน้า {firstPage}{step === 2 && firstPage < pdf.pageCount ? `–${firstPage + 1}` : ''} / {pdf.pageCount}
						</p>
						<Button
							type="button"
							variant="outline"
							size="icon-sm"
							aria-label="หน้าถัดไป"
							disabled={firstPage + step > pdf.pageCount || rendering}
							onclick={() => (firstPage += step)}><ChevronRight class="size-4" /></Button
						>
					</div>{/if}
				<Button variant="outline" size="sm" href={url} download={name}
					><Download class="size-4" />ดาวน์โหลด</Button
				>
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>
