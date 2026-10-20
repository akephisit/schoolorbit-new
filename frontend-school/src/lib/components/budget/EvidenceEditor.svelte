<script lang="ts">
	import { onDestroy } from 'svelte';
	import { uploadFile } from '#lib/api/files.js';
	import { message } from '#lib/budget/drafts.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	let {
		resourceId,
		ids = $bindable([]),
		busy = $bindable(false),
		disabled = false,
		required = false
	}: {
		resourceId: string;
		ids?: string[];
		busy?: boolean;
		disabled?: boolean;
		required?: boolean;
	} = $props();
	let error = $state(''),
		disposed = false;
	onDestroy(() => {
		disposed = true;
	});
	async function upload(event: Event) {
		const target = event.currentTarget as HTMLInputElement,
			files = [...(target.files ?? [])];
		busy = true;
		error = '';
		try {
			for (const file of files) {
				const result = await uploadFile(file, 'budget_evidence', resourceId);
				if (disposed) return;
				ids = [...ids, result.id];
			}
			target.value = '';
		} catch (cause) {
			if (!disposed) error = message(cause);
		} finally {
			if (!disposed) busy = false;
		}
	}
</script>

<div class="space-y-2">
	<Label for={`budget-evidence-${resourceId}`}>แนบหลักฐาน{required ? ' *' : ''}</Label><Input
		id={`budget-evidence-${resourceId}`}
		type="file"
		multiple
		disabled={disabled || busy}
		onchange={upload}
	/>{#if busy}<p role="status" class="text-sm text-muted-foreground">
			กำลังแนบหลักฐาน…
		</p>{/if}{#if error}<PageState
			variant="error"
			title="แนบหลักฐานไม่สำเร็จ"
			description={error}
		/>{/if}{#if ids.length}<ul class="space-y-1">
			{#each ids as fileId, index (fileId)}<li
					class="flex items-center justify-between gap-2 text-sm"
				>
					<span>หลักฐานที่ {index + 1}</span><Button
						type="button"
						size="sm"
						variant="ghost"
						disabled={disabled || busy}
						onclick={() => (ids = ids.filter((id) => id !== fileId))}>นำออก</Button
					>
				</li>{/each}
		</ul>{/if}
</div>
