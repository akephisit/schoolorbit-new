<script lang="ts">
	import { onDestroy } from 'svelte';
	import {
		listLeavePolicyPresets,
		type LeaveKind,
		type LeavePolicyPreset
	} from '#lib/api/staff-leave.js';
	import { LeaveRegion } from '#lib/staff-leave/region.svelte.js';
	import { leaveUnitLabels } from '#lib/staff-leave/presentation.js';
	import { PERSONNEL_TYPE_LABELS } from '#lib/forms/staff-career.js';
	import { PageState, PageSkeleton } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import Choice from './Choice.svelte';

	let {
		kind,
		onapply,
		oncancel
	}: {
		kind: LeaveKind;
		onapply: (preset: LeavePolicyPreset) => void;
		oncancel: () => void;
	} = $props();
	const region = new LeaveRegion<LeavePolicyPreset[]>();
	let selected = $state('');
	const presets = $derived(region.value?.filter((preset) => preset.kind === kind) ?? []);
	const preset = $derived(presets.find((value) => value.id === selected));

	function read() {
		region.retry((signal) => listLeavePolicyPresets({ signal }));
	}
	$effect.pre(read);
	onDestroy(() => region.dispose());
</script>

<section aria-busy={region.loading} class="space-y-4">
	{#if region.error}
		<PageState
			variant="error"
			title="โหลดเกณฑ์อ้างอิงไม่สำเร็จ"
			description={region.error}
			actionLabel="ลองอีกครั้ง"
			onaction={read}
		/>
	{:else if region.loading}
		<PageSkeleton variant="form" />
	{:else if presets.length}
		<Choice
			id="leave-preset"
			label="เกณฑ์อ้างอิง"
			bind:value={selected}
			options={presets.map((value) => ({ value: value.id, label: value.name }))}
		/>
		{#if preset}
			<div class="space-y-3 rounded-lg border bg-muted/20 p-3 text-sm">
				<dl class="grid gap-3 sm:grid-cols-2">
					<div>
						<dt class="text-muted-foreground">ประเภทบุคลากร</dt>
						<dd>
							{preset.personnelTypes.map((value) => PERSONNEL_TYPE_LABELS[value]).join(' · ') ||
								'โรงเรียนเป็นผู้กำหนด'}
						</dd>
					</div>
					<div>
						<dt class="text-muted-foreground">การนับ</dt>
						<dd>
							{leaveUnitLabels[preset.unit]} · {preset.countingMode === 'working_days'
								? 'วันทำงาน'
								: 'วันปฏิทิน'}
						</dd>
					</div>
				</dl>
				<ul class="list-disc space-y-1 pl-5">
					{#each preset.guidance as guidance (guidance)}
						<li>{guidance}</li>
					{/each}
				</ul>
				<a
					class="inline-flex min-h-9 items-center rounded text-primary underline underline-offset-4 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
					href={preset.sourceUrl}
					target="_blank"
					rel="noopener noreferrer">เปิดเอกสารอ้างอิง (แท็บใหม่)</a
				>
			</div>
		{/if}
	{:else}
		<PageState title="ยังไม่มีเกณฑ์อ้างอิงสำหรับประเภทนี้" />
	{/if}
	<div class="flex flex-wrap justify-end gap-2">
		<Button type="button" variant="outline" onclick={oncancel}>ยกเลิก</Button>
		<Button
			type="button"
			disabled={!preset || region.loading}
			onclick={() => {
				if (preset) onapply(preset);
			}}
		>
			ใช้เกณฑ์นี้กับแบบร่าง
		</Button>
	</div>
</section>
