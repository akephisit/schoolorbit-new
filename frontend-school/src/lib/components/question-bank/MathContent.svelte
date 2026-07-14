<script lang="ts">
	import katex from 'katex';
	import 'katex/dist/katex.min.css';

	interface Props {
		latex: string;
		display?: boolean;
		class?: string;
	}

	let { latex, display = false, class: className = '' }: Props = $props();
	let renderOptions = $derived({ latex, display });

	function renderMath(options: { latex: string; display: boolean }) {
		return (node: HTMLElement) => {
			katex.render(options.latex, node, {
				displayMode: options.display,
				throwOnError: false,
				strict: 'warn',
				trust: false
			});
		};
	}
</script>

{#if display}
	<div
		class={['question-math question-math--display', className]}
		{@attach renderMath(renderOptions)}
	></div>
{:else}
	<span
		class={['question-math question-math--inline', className]}
		{@attach renderMath(renderOptions)}
	></span>
{/if}

<style>
	.question-math--inline {
		display: inline;
		line-height: inherit;
		vertical-align: baseline;
	}

	.question-math--inline :global(.katex) {
		font-size: 1em;
		line-height: inherit;
	}

	.question-math--display {
		min-width: 0;
		overflow-x: auto;
		overflow-y: hidden;
	}

	.question-math--display :global(.katex-display) {
		margin-block: 0.5rem;
	}
</style>
