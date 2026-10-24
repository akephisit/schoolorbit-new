<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { uploadFile } from '#lib/api/files.js';
	import { PendingUpload } from '#lib/files/pending-upload.js';
	import PendingFileGuard from '#lib/components/files/PendingFileGuard.svelte';
	import LocalDocumentPreview from '#lib/components/files/LocalDocumentPreview.svelte';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Paperclip } from '@lucide/svelte';
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
	let drafts = $state<PendingUpload[]>([]);
	let baseline = $state(untrack(() => ids.join(',')));
	const dirty = $derived(drafts.length > 0 || ids.join(',') !== baseline);
	export function accept() {
		baseline = ids.join(',');
	}
	let disposed = false,
		generation = 0;
	$effect(() => {
		const owner = resourceId;
		untrack(() => {
			if (drafts.some((draft) => draft.scope.owner !== owner)) {
				generation++;
				for (const draft of drafts) draft.dispose();
				drafts = [];
			}
		});
	});
	onDestroy(() => {
		disposed = true;
		for (const draft of drafts) draft.dispose();
	});
	function select(event: Event) {
		if (disabled || busy) return;
		const input = event.currentTarget as HTMLInputElement;
		drafts = [
			...drafts,
			...Array.from(input.files ?? []).map(
				(file) =>
					new PendingUpload(
						file,
						{
							purpose: 'budget_evidence',
							resourceId,
							owner: resourceId
						},
						uploadFile
					)
			)
		];
		input.value = '';
	}
	export async function commit(): Promise<void> {
		const owner = resourceId,
			revision = generation;
		busy = true;
		try {
			for (const draft of [...drafts]) {
				const id = await draft.commit();
				if (disposed || owner !== resourceId || revision !== generation)
					throw new Error('คำขอหลักฐานเปลี่ยนแล้ว');
				if (!ids.includes(id)) ids = [...ids, id];
				drafts = drafts.filter((value) => value !== draft);
				draft.dispose();
			}
		} finally {
			if (!disposed && revision === generation) busy = false;
		}
	}
</script>

<PendingFileGuard {dirty} pending={dirty && (busy || disabled)} />
<section class="space-y-3 border-t pt-6" aria-label="หลักฐานประกอบ">
	<Label for={`budget-evidence-${resourceId}`}
		><Paperclip class="size-4 text-muted-foreground" aria-hidden="true" />แนบหลักฐาน{required
			? ' *'
			: ''}</Label
	><Input
		id={`budget-evidence-${resourceId}`}
		type="file"
		multiple
		disabled={disabled || busy}
		onchange={select}
	/>
	<p class="text-sm text-muted-foreground">แนบเอกสารประกอบ{required ? 'การทำรายการ' : 'ถ้ามี'}</p>
	{#each drafts as draft (draft)}
		<div class="space-y-2">
			<LocalDocumentPreview file={draft.file} /><Button
				type="button"
				size="sm"
				variant="ghost"
				disabled={disabled || busy}
				onclick={() => {
					draft.dispose();
					drafts = drafts.filter((value) => value !== draft);
				}}>นำไฟล์ที่เลือกออก</Button
			>
		</div>
	{/each}
	{#if busy}<p role="status" class="text-sm text-muted-foreground">กำลังแนบหลักฐาน…</p>{/if}
	{#if ids.length}<ul class="divide-y rounded-lg border px-3">
			{#each ids as fileId, index (fileId)}<li
					class="flex items-center justify-between gap-2 py-2 text-sm"
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
</section>
