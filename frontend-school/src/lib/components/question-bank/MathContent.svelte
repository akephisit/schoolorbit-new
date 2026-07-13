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

<span class={className} {@attach renderMath(renderOptions)}></span>
