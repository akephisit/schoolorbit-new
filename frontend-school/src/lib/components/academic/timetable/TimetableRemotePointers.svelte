<script lang="ts">
	import { untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import {
		containsPointer,
		interpolatePointer,
		intersectRects,
		projectPointer,
		type ClientRect,
		type ClientPoint,
		type RelativePoint
	} from '#lib/academic/timetable/collaboration-geometry.js';
	import type { TimetableRemotePointer } from '#lib/academic/timetable/collaboration-presentation.js';

	let { pointers, root }: { pointers: TimetableRemotePointer[]; root: HTMLElement | null } =
		$props();
	type Motion = {
		from: RelativePoint;
		to: RelativePoint;
		started: number;
		subjectKey: string;
		contextKey: string;
		subject: HTMLElement;
		fromClientPoint: ClientPoint | null;
	};
	type VisiblePointer = { pointer: TimetableRemotePointer; x: number; y: number };
	let visible = $state.raw<VisiblePointer[]>([]);
	const motions = new SvelteMap<string, Motion>();
	let invalidate = () => {};

	function visibleRect(element: HTMLElement, includeSubject = true): ClientRect | null {
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
			if (parent === element && !includeSubject) continue;
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

	function motionPoint(motion: Motion, now: number): ClientPoint | null {
		const rect = motion.subject.getBoundingClientRect();
		if (motion.fromClientPoint) {
			const target = projectPointer(motion.to, rect);
			if (!target) return null;
			const progress = Math.max(0, Math.min(1, (now - motion.started) / 66));
			return {
				x: motion.fromClientPoint.x + (target.x - motion.fromClientPoint.x) * progress,
				y: motion.fromClientPoint.y + (target.y - motion.fromClientPoint.y) * progress
			};
		}
		return projectPointer(interpolatePointer(motion.from, motion.to, now - motion.started), rect);
	}

	$effect(() => {
		const current = pointers;
		untrack(() => {
			const now = performance.now();
			const ids = new Set(current.map((pointer) => pointer.connectionId));
			for (const id of motions.keys()) if (!ids.has(id)) motions.delete(id);
			for (const pointer of current) {
				const previous = motions.get(pointer.connectionId);
				const sameContext = previous?.contextKey === pointer.contextKey;
				const from =
					previous?.subjectKey === pointer.subjectKey
						? interpolatePointer(previous.from, previous.to, now - previous.started)
						: pointer.position;
				const previousPoint =
					sameContext && previous?.subject.isConnected ? motionPoint(previous, now) : null;
				const targetPoint = projectPointer(
					pointer.position,
					pointer.subject.getBoundingClientRect()
				);
				const clipped = previous ? visibleRect(previous.subject, !previous.fromClientPoint) : null;
				const crossing =
					previous &&
					(previous.subjectKey !== pointer.subjectKey || previous.fromClientPoint !== null);
				const fromClientPoint =
					crossing &&
					previousPoint &&
					targetPoint &&
					clipped &&
					containsPointer(clipped, previousPoint) &&
					Math.hypot(targetPoint.x - previousPoint.x, targetPoint.y - previousPoint.y) <= 300
						? previousPoint
						: null;
				motions.set(pointer.connectionId, {
					from,
					to: pointer.position,
					started: now,
					subjectKey: pointer.subjectKey,
					contextKey: pointer.contextKey,
					subject: pointer.subject,
					fromClientPoint
				});
			}
			invalidate();
		});
	});

	$effect(() => {
		const region = root;
		const subjects = pointers.map((pointer) => pointer.subject);
		if (!region) {
			visible = [];
			return;
		}
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
				moving ||= elapsed < 66;
				const point = motion
					? motionPoint(motion, now)
					: projectPointer(pointer.position, pointer.subject.getBoundingClientRect());
				const clipped = visibleRect(pointer.subject, !(motion?.fromClientPoint && elapsed < 66));
				if (point && clipped && containsPointer(clipped, point)) next.push({ pointer, ...point });
			}
			visible = next;
			if (moving) schedule();
		}
		function schedule() {
			if (!disposed && !frame) frame = requestAnimationFrame(draw);
		}
		invalidate = schedule;
		function layoutChanged() {
			for (const [id, motion] of motions)
				motions.set(id, {
					...motion,
					from: motion.to,
					fromClientPoint: null,
					started: performance.now() - 66
				});
			schedule();
		}
		const observer = new ResizeObserver(layoutChanged);
		observer.observe(region);
		for (const subject of subjects) observer.observe(subject);
		window.addEventListener('resize', layoutChanged);
		window.addEventListener('scroll', layoutChanged, true);
		window.visualViewport?.addEventListener('resize', layoutChanged);
		window.visualViewport?.addEventListener('scroll', layoutChanged);
		schedule();
		return () => {
			disposed = true;
			if (frame) cancelAnimationFrame(frame);
			observer.disconnect();
			window.removeEventListener('resize', layoutChanged);
			window.removeEventListener('scroll', layoutChanged, true);
			window.visualViewport?.removeEventListener('resize', layoutChanged);
			window.visualViewport?.removeEventListener('scroll', layoutChanged);
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
				class="absolute left-3 top-4 max-w-44 truncate rounded-md border bg-background px-2 py-0.5 text-xs text-foreground shadow-sm"
				style:border-color={pointer.color}>{pointer.name}</span
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
