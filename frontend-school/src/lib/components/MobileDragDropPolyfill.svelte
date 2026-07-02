<script lang="ts" module>
	import 'mobile-drag-drop/default.css';

	let initializationPromise: Promise<void> | null = null;

	function initializeMobileDragDrop() {
		initializationPromise ??= (async () => {
			const { polyfill } = await import('mobile-drag-drop');
			const { scrollBehaviourDragImageTranslateOverride } =
				await import('mobile-drag-drop/scroll-behaviour');

			polyfill({
				dragImageTranslateOverride: scrollBehaviourDragImageTranslateOverride,
				holdToDrag: 200
			});
		})();

		return initializationPromise;
	}
</script>

<script lang="ts">
	import { onMount } from 'svelte';

	onMount(() => {
		void initializeMobileDragDrop().catch((error) => {
			console.warn('Mobile drag-and-drop polyfill failed to initialize', error);
		});
	});
</script>
