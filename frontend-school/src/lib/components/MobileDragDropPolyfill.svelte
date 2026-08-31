<script lang="ts" module>
	import 'mobile-drag-drop/default.css';

	let initializationPromise: Promise<void> | null = null;

	function hasDeliberateDragTarget(event: TouchEvent): boolean {
		if (event.touches.length !== 1 || !(event.target instanceof Element)) return false;

		const timetableCard = event.target.closest('[data-timetable-lesson-card]');
		if (timetableCard) {
			return event.target.closest('[data-timetable-drag-handle="true"]') !== null;
		}

		return event.composedPath().some((node) => node instanceof HTMLElement && node.draggable);
	}

	function initializeMobileDragDrop() {
		initializationPromise ??= (async () => {
			const { polyfill } = await import('mobile-drag-drop');
			const { scrollBehaviourDragImageTranslateOverride } =
				await import('mobile-drag-drop/scroll-behaviour');

			polyfill({
				dragImageTranslateOverride: scrollBehaviourDragImageTranslateOverride,
				dragStartConditionOverride: hasDeliberateDragTarget,
				holdToDrag: 250
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
