<script lang="ts">
	import { untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		containsPointer,
		interpolatePointer,
		intersectRects,
		projectPointer,
		type ClientRect,
		type RelativePoint
	} from '#lib/academic/timetable/collaboration-geometry.js';
	import type { TimetableRemotePointer } from '#lib/academic/timetable/collaboration-presentation.js';

	let { pointers, root }: { pointers: TimetableRemotePointer[]; root: HTMLElement | null } =
		$props();
	type Motion = { from: RelativePoint; to: RelativePoint; started: number; subjectKey: string };
	type VisiblePointer = { pointer: TimetableRemotePointer; x: number; y: number };
	let visible = $state.raw<VisiblePointer[]>([]);
	const motions = new SvelteMap<string, Motion>();
	let invalidate = () => {};

	function visibleRect(element: HTMLElement): ClientRect | null {
		let clipped: ClientRect | null = {
			left: 0,
			top: 0,
			width: window.innerWidth,
			height: window.innerHeight
		};
		for (
			let parent: HTMLElement | null = element;
			parent && clipped;
			parent = parent.parentElement
		) {
			const style = getComputedStyle(parent);
			if (
				parent !== element &&
				!/(auto|scroll|hidden|clip)/u.test(`${style.overflowX} ${style.overflowY}`)
			)
				continue;
			clipped = intersectRects(clipped, parent.getBoundingClientRect());
		}
		return clipped;
	}

	$effect(() => {
		const current = pointers;
		untrack(() => {
			const now = performance.now();
			const ids = new Set(current.map((pointer) => pointer.connectionId));
			for (const id of motions.keys()) if (!ids.has(id)) motions.delete(id);
			for (const pointer of current) {
				const previous = motions.get(pointer.connectionId);
				const from =
					previous?.subjectKey === pointer.subjectKey
						? interpolatePointer(previous.from, previous.to, now - previous.started)
						: pointer.position;
				motions.set(pointer.connectionId, {
					from,
					to: pointer.position,
					started: now,
					subjectKey: pointer.subjectKey
				});
			}
			invalidate();
		});
	});

	$effect(() => {
		const region = root;
		const subjects = pointers.map((pointer) => pointer.subject);
		if (!region) return;
		let frame = 0;
		let disposed = false;
		function draw(now: number) {
			frame = 0;
			if (disposed) return;
			let moving = false;
			const next: VisiblePointer[] = [];
			for (const pointer of pointers) {
				if (!pointer.subject.isConnected || !region?.contains(pointer.subject)) continue;
				const motion = motions.get(pointer.connectionId);
				const elapsed = motion ? now - motion.started : 66;
				const position = motion
					? interpolatePointer(motion.from, motion.to, elapsed)
					: pointer.position;
				moving ||= elapsed < 66;
				const point = projectPointer(position, pointer.subject.getBoundingClientRect());
				const clipped = visibleRect(pointer.subject);
				if (point && clipped && containsPointer(clipped, point)) next.push({ pointer, ...point });
			}
			visible = next;
			if (moving) schedule();
		}
		function schedule() {
			if (!disposed && !frame) frame = requestAnimationFrame(draw);
		}
		invalidate = schedule;
		const observer = new ResizeObserver(schedule);
		observer.observe(region);
		for (const subject of subjects) observer.observe(subject);
		window.addEventListener('resize', schedule);
		window.addEventListener('scroll', schedule, true);
		window.visualViewport?.addEventListener('resize', schedule);
		window.visualViewport?.addEventListener('scroll', schedule);
		schedule();
		return () => {
			disposed = true;
			if (frame) cancelAnimationFrame(frame);
			observer.disconnect();
			window.removeEventListener('resize', schedule);
			window.removeEventListener('scroll', schedule, true);
			window.visualViewport?.removeEventListener('resize', schedule);
			window.visualViewport?.removeEventListener('scroll', schedule);
		};
	});
</script>

<div
	class="pointer-events-none fixed inset-0 z-40 overflow-hidden"
	aria-hidden="true"
	data-timetable-remote-pointers
>
	{#each visible as { pointer, x, y } (pointer.connectionId)}
		<div
			class="absolute left-0 top-0"
			style:transform={`translate3d(${x}px, ${y}px, 0)`}
			data-timetable-remote-cursor={pointer.connectionId}
		>
			<svg width="16" height="20" viewBox="0 0 16 20" fill={pointer.color} class="drop-shadow-sm"
				><path
					d="M1 1L14 12H8L5 18L1 1Z"
					stroke="white"
					stroke-width="1.5"
					stroke-linejoin="round"
				/></svg
			>
			<span
				class="absolute left-3 top-4 max-w-44 truncate rounded-md px-2 py-0.5 text-xs text-white shadow-sm"
				style:background-color={pointer.color}>{pointer.name}</span
			>
			{#if pointer.ghost}
				<div
					class="absolute left-5 top-11 w-40 rounded-lg border-2 bg-background/95 p-2 text-foreground shadow-lg"
					style:border-color={pointer.color}
					data-timetable-remote-ghost
				>
					{#if pointer.ghost.code}<p class="truncate text-[10px] font-semibold">
							{pointer.ghost.code}
						</p>{/if}
					<p class="line-clamp-2 text-xs font-semibold">{pointer.ghost.title}</p>
				</div>
			{/if}
		</div>
	{/each}
</div>
