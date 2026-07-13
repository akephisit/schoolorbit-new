<script lang="ts">
	import type { QuestionFile, RichContent } from '$lib/api/questionBank';
	import MathContent from './MathContent.svelte';

	interface Props {
		content?: RichContent | null;
		files?: QuestionFile[];
		compact?: boolean;
	}

	let { content = null, files = [], compact = false }: Props = $props();
	let fileUrls = $derived(new Map(files.map((file) => [file.id, file.thumbnailUrl ?? file.url])));
</script>

{#if content?.blocks.length}
	<div class:space-y-2={!compact} class="min-w-0">
		{#each content.blocks as block, index (`${block.type}-${index}`)}
			{#if block.type === 'paragraph'}
				<p class:line-clamp-2={compact} class="whitespace-pre-wrap break-words">{block.text}</p>
			{:else if block.type === 'math'}
				<div class:overflow-x-auto={!compact} class:truncate={compact}>
					<MathContent latex={block.latex} display={block.display && !compact} />
				</div>
			{:else if fileUrls.get(block.fileId)}
				<figure class="space-y-1">
					<img
						src={fileUrls.get(block.fileId)}
						alt={block.altText ?? ''}
						class={compact
							? 'max-h-24 max-w-full rounded-md border object-contain'
							: 'max-h-96 max-w-full rounded-md border object-contain'}
					/>
					{#if block.caption && !compact}
						<figcaption class="text-sm text-muted-foreground">{block.caption}</figcaption>
					{/if}
				</figure>
			{/if}
		{/each}
	</div>
{/if}
