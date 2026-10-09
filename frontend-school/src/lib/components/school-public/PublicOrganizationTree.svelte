<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import type { PublicOrganizationNode } from '#lib/school-public/organization.js';
	import {
		groupPublicOrganizationMembers,
		administrationTree
	} from '#lib/school-public/organization.js';
	import { fitChart, zoomChart, type ChartTransform } from '#lib/school-public/chart-transform.js';
	import { BACKEND_URL } from '#lib/api/client.js';
	import { Avatar } from '#lib/components/ui/avatar/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Building2, ChevronDown, Plus, Minus, Maximize } from '@lucide/svelte';
	let { nodes }: { nodes: PublicOrganizationNode[] } = $props();
	const visibleNodes = $derived(administrationTree(nodes));
	let chart: HTMLDivElement, content: HTMLDivElement;
	let view = $state<ChartTransform>({ x: 0, y: 0, scale: 1 });
	let dragging = $state(false);
	const pointers = new SvelteMap<number, { x: number; y: number }>();
	let start: ChartTransform = { x: 0, y: 0, scale: 1 },
		startPoint = { x: 0, y: 0 },
		distance = 0;
	function fit() {
		if (chart && content)
			view = fitChart(
				chart.clientWidth,
				chart.clientHeight,
				content.offsetWidth,
				content.offsetHeight
			);
	}
	function zoom(factor: number, x = chart.clientWidth / 2, y = chart.clientHeight / 2) {
		view = zoomChart(view, view.scale * factor, { x, y });
	}
	function local(event: PointerEvent) {
		const rect = chart.getBoundingClientRect();
		return { x: event.clientX - rect.left, y: event.clientY - rect.top };
	}
	function beginGesture() {
		start = { ...view };
		const values = [...pointers.values()];
		startPoint =
			values.length > 1
				? { x: (values[0].x + values[1].x) / 2, y: (values[0].y + values[1].y) / 2 }
				: values[0];
		distance =
			values.length > 1 ? Math.hypot(values[1].x - values[0].x, values[1].y - values[0].y) : 0;
	}
	function pointerDown(event: PointerEvent) {
		if (
			event.button !== 0 ||
			(event.target instanceof Element &&
				event.target.closest('summary, button:not([data-chart-control=pan]), a'))
		)
			return;
		pointers.set(event.pointerId, local(event));
		chart.setPointerCapture(event.pointerId);
		dragging = true;
		beginGesture();
	}
	function pointerMove(event: PointerEvent) {
		if (!pointers.has(event.pointerId)) return;
		pointers.set(event.pointerId, local(event));
		const values = [...pointers.values()];
		if (values.length > 1) {
			const center = { x: (values[0].x + values[1].x) / 2, y: (values[0].y + values[1].y) / 2 };
			const ratio =
				Math.hypot(values[1].x - values[0].x, values[1].y - values[0].y) / Math.max(1, distance);
			const scaled = zoomChart(start, start.scale * ratio, startPoint);
			view = {
				...scaled,
				x: scaled.x + center.x - startPoint.x,
				y: scaled.y + center.y - startPoint.y
			};
		} else
			view = {
				...start,
				x: start.x + values[0].x - startPoint.x,
				y: start.y + values[0].y - startPoint.y
			};
	}
	function pointerEnd(event: PointerEvent) {
		pointers.delete(event.pointerId);
		dragging = pointers.size > 0;
		if (dragging) beginGesture();
	}
	function wheel(event: WheelEvent) {
		event.preventDefault();
		const rect = chart.getBoundingClientRect();
		zoom(
			Math.exp(-Math.max(-100, Math.min(100, event.deltaY)) * 0.002),
			event.clientX - rect.left,
			event.clientY - rect.top
		);
	}
	function keyDown(event: KeyboardEvent) {
		if (!(event.target instanceof HTMLButtonElement) || event.target.dataset.chartControl !== 'pan')
			return;
		if (['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) {
			event.preventDefault();
			view = {
				...view,
				x: view.x + (event.key === 'ArrowLeft' ? 40 : event.key === 'ArrowRight' ? -40 : 0),
				y: view.y + (event.key === 'ArrowUp' ? 40 : event.key === 'ArrowDown' ? -40 : 0)
			};
		} else if (event.key === '+' || event.key === '=') zoom(1.2);
		else if (event.key === '-') zoom(1 / 1.2);
		else if (event.key === '0') fit();
	}
	async function refit() {
		await tick();
		fit();
	}
	function keepFocusedNodeVisible(event: FocusEvent) {
		const target = event.target;
		if (
			!(target instanceof HTMLElement) ||
			target === chart ||
			target.dataset.chartControl === 'pan' ||
			!target.matches(':focus-visible')
		)
			return;
		const node = target.getBoundingClientRect(),
			bounds = chart.getBoundingClientRect();
		const dx =
			node.left < bounds.left + 16
				? bounds.left + 16 - node.left
				: node.right > bounds.right - 16
					? bounds.right - 16 - node.right
					: 0;
		const dy =
			node.top < bounds.top + 16
				? bounds.top + 16 - node.top
				: node.bottom > bounds.bottom - 16
					? bounds.bottom - 16 - node.bottom
					: 0;
		view = { ...view, x: view.x + dx, y: view.y + dy };
	}
	onMount(() => {
		fit();
		const observer = new ResizeObserver(fit);
		observer.observe(chart);
		observer.observe(content);
		chart.addEventListener('wheel', wheel, { passive: false });
		chart.addEventListener('pointerdown', pointerDown);
		chart.addEventListener('pointermove', pointerMove);
		chart.addEventListener('pointerup', pointerEnd);
		chart.addEventListener('pointercancel', pointerEnd);
		chart.addEventListener('lostpointercapture', pointerEnd);
		chart.addEventListener('focusin', keepFocusedNodeVisible);
		return () => {
			observer.disconnect();
			chart.removeEventListener('wheel', wheel);
			chart.removeEventListener('pointerdown', pointerDown);
			chart.removeEventListener('pointermove', pointerMove);
			chart.removeEventListener('pointerup', pointerEnd);
			chart.removeEventListener('pointercancel', pointerEnd);
			chart.removeEventListener('lostpointercapture', pointerEnd);
			chart.removeEventListener('focusin', keepFocusedNodeVisible);
			pointers.clear();
		};
	});
</script>

{#snippet branch(items: PublicOrganizationNode[], depth: number)}
	<ul class="flex w-max min-w-full justify-center">
		{#each items as unit (unit.id)}
			<li class="relative flex flex-col items-center px-3" class:org-node={depth > 0}>
				<details open={depth < 2} ontoggle={refit} class="flex flex-col items-center">
					<summary
						class="public-surface flex w-56 cursor-pointer list-none items-start gap-2 rounded-2xl border bg-card p-3 shadow-sm"
					>
						<Building2 class="mt-0.5 size-4 shrink-0 text-primary" />
						<div class="min-w-0 flex-1">
							<h3 class="text-sm font-semibold">{unit.name}</h3>
							<p class="mt-1 text-xs text-muted-foreground">
								{unit.members.length} คน{#if unit.children.length}
									· {unit.children.length} หน่วยงานย่อย{/if}
							</p>
						</div>
						<ChevronDown
							class="mt-0.5 size-4 shrink-0 text-muted-foreground transition-transform unit-chevron"
						/>
					</summary>
					<div class="flex flex-col items-center">
						{#if unit.members.length}
							<div class="mt-2 w-56 space-y-3 rounded-2xl border bg-card p-3 shadow-sm">
								{#each groupPublicOrganizationMembers(unit.members) as group (group.code)}
									<section aria-label={group.label}>
										<h4
											class="mb-2 flex items-center justify-between gap-2 text-xs font-medium text-primary"
										>
											<span>{group.label}</span><span class="font-normal text-muted-foreground"
												>{group.members.length} คน</span
											>
										</h4>
										<ul class="space-y-2">
											{#each group.members as member, index (index)}
												<li class="flex items-center gap-2">
													<Avatar
														src={member.avatarUrl ? `${BACKEND_URL}${member.avatarUrl}` : null}
														alt={`รูป ${member.name}`}
														size="sm"
														class="shrink-0"
													/>
													<div class="min-w-0">
														<p class="break-words text-xs font-medium">{member.name}</p>
														{#if member.positionTitle}<p
																class="mt-0.5 text-xs text-muted-foreground"
															>
																{member.positionTitle}
															</p>{/if}
													</div>
												</li>
											{/each}
										</ul>
									</section>
								{/each}
							</div>
						{:else if !unit.children.length}
							<p class="mt-2 w-56 text-center text-xs text-muted-foreground">
								ยังไม่มีสมาชิกในหน่วยงานนี้
							</p>
						{/if}
						{#if unit.children.length}
							<div class="h-6 border-l border-border" aria-hidden="true"></div>
							{@render branch(unit.children, depth + 1)}
						{/if}
					</div>
				</details>
			</li>
		{/each}
	</ul>
{/snippet}

<div class="overflow-hidden rounded-2xl border bg-muted/20">
	<div class="flex flex-wrap items-center justify-between gap-3 border-b bg-card p-3">
		<p class="text-xs text-muted-foreground" id="organization-chart-help">
			ลากผังเพื่อเลื่อน · ล้อเมาส์หรือบีบนิ้วเพื่อซูม · ปุ่มลูกศรเพื่อเลื่อน และ 0 เพื่อพอดีจอ
		</p>
		<div class="flex items-center gap-1">
			<Button
				size="icon"
				variant="outline"
				aria-label="ซูมออก"
				onclick={() => zoom(1 / 1.2)}
				disabled={view.scale <= 0.08}><Minus class="size-4" /></Button
			><output class="w-14 text-center text-xs tabular-nums" aria-label="ระดับซูม"
				>{Math.round(view.scale * 100)}%</output
			><Button
				size="icon"
				variant="outline"
				aria-label="ซูมเข้า"
				onclick={() => zoom(1.2)}
				disabled={view.scale >= 2.5}><Plus class="size-4" /></Button
			><Button size="sm" variant="outline" onclick={fit}><Maximize class="size-4" />พอดีจอ</Button>
		</div>
	</div>
	<div
		bind:this={chart}
		class="relative h-[440px] touch-none overflow-hidden select-none outline-none focus-visible:ring-2 focus-visible:ring-primary sm:h-[560px]"
		class:cursor-grabbing={dragging}
		class:cursor-grab={!dragging}
		role="region"
		aria-label="แผนผังบุคลากรและหน่วยงาน"
		aria-describedby="organization-chart-help"
		data-testid="public-organization-chart"
	>
		<button
			type="button"
			class="absolute inset-0 focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-primary"
			data-chart-control="pan"
			aria-label="เลื่อนและซูมผังบริหาร"
			aria-describedby="organization-chart-help"
			onkeydown={keyDown}
			onclick={(event) => {
				if (event.detail === 0) fit();
			}}
		></button>
		<div
			bind:this={content}
			class="absolute left-0 top-0 w-max origin-top-left p-4"
			data-testid="organization-chart-content"
			style:transform={`translate(${view.x}px, ${view.y}px) scale(${view.scale})`}
		>
			{@render branch(visibleNodes, 0)}
		</div>
	</div>
</div>

<style>
	:global(details[open] > summary .unit-chevron) {
		transform: rotate(180deg);
	}
	.org-node {
		padding-top: 1.5rem;
	}
	.org-node::before {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		border-top: 1px solid var(--border);
	}
	.org-node::after {
		content: '';
		position: absolute;
		top: 0;
		left: 50%;
		height: 1.5rem;
		border-left: 1px solid var(--border);
	}
	.org-node:first-child::before {
		left: 50%;
	}
	.org-node:last-child::before {
		right: 50%;
	}
	.org-node:only-child::before {
		border-top: 0;
	}
</style>
