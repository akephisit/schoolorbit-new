<script lang="ts">
	import type { QuestionFile, RichContent, RichInlineNode } from '$lib/api/questionBank';
	import MathContent from './MathContent.svelte';

	interface Props {
		content?: RichContent | null;
		files?: QuestionFile[];
		compact?: boolean;
	}

	let { content = null, files = [], compact = false }: Props = $props();
	let fileUrls = $derived(new Map(files.map((file) => [file.id, file.thumbnailUrl ?? file.url])));

	function hasMark(node: Extract<RichInlineNode, { type: 'text' }>, type: 'bold' | 'italic') {
		return node.marks?.some((mark) => mark.type === type) ?? false;
	}

	function imageWidth(value: number) {
		if (!Number.isFinite(value)) return 60;
		return Math.min(100, Math.max(10, Math.round(value)));
	}
</script>

{#if content?.document.content.length}
	<div class:space-y-2={!compact} class="min-w-0">
		{#each content.document.content as block, index (`${block.type}-${index}`)}
			{#if block.type === 'paragraph'}
				<p class:line-clamp-2={compact} class="whitespace-pre-wrap break-words">
					{#each block.content ?? [] as node, inlineIndex (`${node.type}-${inlineIndex}`)}
						{#if node.type === 'text'}
							{#if hasMark(node, 'bold') && hasMark(node, 'italic')}
								<strong><em>{node.text}</em></strong>
							{:else if hasMark(node, 'bold')}
								<strong>{node.text}</strong>
							{:else if hasMark(node, 'italic')}
								<em>{node.text}</em>
							{:else}
								{node.text}
							{/if}
						{:else if node.type === 'inline_math'}
							<MathContent latex={node.attrs.latex} class="mx-0.5 inline-block align-middle" />
						{:else}
							<br />
						{/if}
					{/each}
				</p>
			{:else if block.type === 'math_block'}
				<div class:overflow-x-auto={!compact} class:truncate={compact}>
					<MathContent latex={block.attrs.latex} display={!compact} />
				</div>
			{:else if fileUrls.get(block.attrs.fileId)}
				<figure
					class="space-y-1"
					class:mr-auto={block.attrs.alignment === 'left'}
					class:mx-auto={block.attrs.alignment === 'center'}
					class:ml-auto={block.attrs.alignment === 'right'}
					style:width={compact ? '100%' : `${imageWidth(block.attrs.widthPercent)}%`}
				>
					<img
						src={fileUrls.get(block.attrs.fileId)}
						alt={block.attrs.altText ?? ''}
						class={compact
							? 'max-h-24 max-w-full rounded-md border object-contain'
							: 'max-h-96 max-w-full rounded-md border object-contain'}
					/>
					{#if block.attrs.caption && !compact}
						<figcaption class="text-sm text-muted-foreground">{block.attrs.caption}</figcaption>
					{/if}
				</figure>
			{/if}
		{/each}
	</div>
{/if}
