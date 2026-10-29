<script lang="ts">
	import { mergeProps } from 'bits-ui';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import type { TimetablePlacementReasons } from '#lib/academic/timetable/placement-reasons.js';

	let {
		reasons,
		active = false
	}: {
		reasons: TimetablePlacementReasons;
		active?: boolean;
	} = $props();
	let trigger = $state<HTMLButtonElement | null>(null);
	let restoreTriggerFocus = false;
	let selected = $state(false);
	let hovered = $state(false);
	const open = $derived(selected || hovered || active);

	function handlePopupKeydown(event: KeyboardEvent): void {
		if (event.key !== 'Escape' || active) return;
		event.stopPropagation();
		restoreTriggerFocus = true;
		selected = false;
		hovered = false;
	}
</script>

<Popover.Root
	{open}
	onOpenChange={(value) => {
		selected = value;
		if (!value) hovered = false;
	}}
>
	<Popover.Trigger
		bind:ref={trigger}
		onkeydown={(event) => {
			if (open) handlePopupKeydown(event);
		}}
		class="absolute inset-1.5 z-30 flex min-w-0 flex-col items-start justify-center gap-1 rounded-md border border-destructive/30 bg-background/95 p-2 text-left text-xs leading-relaxed text-foreground shadow-sm outline-none focus-visible:ring-2 focus-visible:ring-primary"
		aria-label={`วางไม่ได้: ${[reasons.primary, reasons.lesson, reasons.context].filter(Boolean).join(' · ')}${reasons.details.length > 1 ? ` และอีก ${reasons.details.length - 1} เหตุผล` : ''}`}
		data-timetable-placement-reason
		onpointerenter={(event) => {
			if (event.pointerType !== 'touch') hovered = true;
		}}
		onpointerleave={() => {
			hovered = false;
		}}
	>
		<span class="flex w-full min-w-0 items-center gap-1 font-semibold text-destructive">
			<span class="line-clamp-1 min-w-0 flex-1">{reasons.primary}</span>
			{#if reasons.details.length > 1}<span class="shrink-0">+{reasons.details.length - 1}</span
				>{/if}
		</span>

		{#if reasons.lesson}<span class="line-clamp-1 w-full">{reasons.lesson}</span>{/if}
		{#if reasons.context}<span class="line-clamp-1 w-full text-muted-foreground"
				>{reasons.context}</span
			>{/if}
	</Popover.Trigger>
	<Popover.Content
		class="max-h-72 w-80 max-w-[calc(100vw-2rem)] overflow-auto p-3 text-sm"
		align="start"
		side="top"
		role="dialog"
		aria-label="เหตุผลที่วางคาบไม่ได้"
		onOpenAutoFocus={(event) => event.preventDefault()}
		onCloseAutoFocus={(event) => {
			event.preventDefault();
			if (restoreTriggerFocus) {
				restoreTriggerFocus = false;
				if (trigger?.isConnected) trigger.focus({ preventScroll: true });
			}
		}}
	>
		<!-- The floating wrapper also needs transparency throughout the drag and exit transition. -->
		{#snippet child({ props, wrapperProps })}
			<div {...wrapperProps} style:pointer-events={active || !open ? 'none' : 'auto'}>
				<div
					{...mergeProps(props, { onkeydown: handlePopupKeydown })}
					style:pointer-events={active || !open ? 'none' : 'auto'}
				>
					<p class="mb-2 font-semibold text-destructive">เหตุผลที่วางคาบไม่ได้</p>
					<ul class="space-y-2">
						{#each reasons.details as detail, index (index)}
							<li class="break-words">{detail}</li>
						{/each}
					</ul>
				</div>
			</div>
		{/snippet}
	</Popover.Content>
</Popover.Root>
