<script lang="ts">
	import { onDestroy } from 'svelte';
	import { downloadFile } from '#lib/api/files.js';
	import { saveDownload } from '#lib/budget/download.js';
	import { message } from '#lib/budget/drafts.js';
	import { LoadingButton } from '#lib/components/app-state/index.js';
	import { FileText } from '@lucide/svelte';
	import { PageState } from '#lib/components/app-state/index.js';
	let { ids, resourceId }: { ids: string[]; resourceId: string } = $props();
	let pending = $state(''),
		error = $state('');
	const controller = new AbortController();
	onDestroy(() => controller.abort());
	async function download(fileId: string, index: number) {
		pending = fileId;
		error = '';
		try {
			const blob = await downloadFile(fileId, resourceId, controller.signal);
			if (!controller.signal.aborted) saveDownload(blob, `หลักฐาน-${index + 1}`);
		} catch (cause) {
			if (!controller.signal.aborted) error = message(cause);
		} finally {
			if (!controller.signal.aborted) pending = '';
		}
	}
</script>

{#if ids.length}<div class="space-y-2">
		<p class="text-sm font-medium">หลักฐาน</p>
		<div class="flex flex-wrap gap-2">
			{#each ids as fileId, index (fileId)}<LoadingButton
					type="button"
					size="sm"
					variant="outline"
					loading={pending === fileId}
					onclick={() => download(fileId, index)}
					><FileText class="size-4" aria-hidden="true" />เปิดหลักฐานที่ {index + 1}</LoadingButton
				>{/each}
		</div>
		{#if error}<PageState variant="error" title="เปิดหลักฐานไม่สำเร็จ" description={error} />{/if}
	</div>{/if}
