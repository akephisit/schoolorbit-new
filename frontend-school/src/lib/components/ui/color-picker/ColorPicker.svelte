<script lang="ts">
	import { Check, ChevronDown } from '@lucide/svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import { Slider } from '#lib/components/ui/slider/index.js';
	import { hexToRgb, rgbToHex } from './color.js';

	let {
		id,
		value,
		text,
		disabled = false,
		invalid = false,
		describedby,
		onTextChange
	}: {
		id: string;
		value: string;
		text: string;
		disabled?: boolean;
		invalid?: boolean;
		describedby?: string;
		onTextChange: (value: string) => void;
	} = $props();
	const uid = $props.id();
	const channels = $derived(hexToRgb(value));
	const presets = [
		['เขียวมิ้นต์อ่อน', '#d1fae5'],
		['ฟ้าอ่อน', '#dbeafe'],
		['ม่วงอ่อน', '#ede9fe'],
		['ชมพูอ่อน', '#fce7f3'],
		['พีชอ่อน', '#ffedd5'],
		['เหลืองอ่อน', '#fef9c3'],
		['เขียวมิ้นต์', '#6ee7b7'],
		['ฟ้า', '#93c5fd'],
		['ม่วง', '#c4b5fd'],
		['ชมพู', '#f9a8d4'],
		['ส้ม', '#fdba74'],
		['เหลือง', '#fde047'],
		['เขียวอมฟ้า', '#167166'],
		['น้ำเงิน', '#245fc2'],
		['ม่วงเข้ม', '#7850a5'],
		['เทา', '#64748b'],
		['ขาว', '#ffffff'],
		['ดำ', '#000000']
	] as const;
	const channelLabels = ['แดง', 'เขียว', 'น้ำเงิน'];
	function changeChannel(index: number, channel: number) {
		if (disabled) return;
		const next = hexToRgb(value);
		next[index] = channel;
		onTextChange(rgbToHex(next));
	}
</script>

<div class="flex min-w-0 gap-2" data-slot="color-picker">
	<Popover.Root>
		<Popover.Trigger {disabled}>
			{#snippet child({ props })}
				<Button
					{...props}
					{disabled}
					variant="outline"
					class="gap-2 px-2.5"
					aria-label="เลือกสีหลัก"
				>
					<span class="size-5 rounded-sm border border-foreground/30" style:background={value}
					></span>
					<ChevronDown class="size-3.5 text-muted-foreground" />
				</Button>
			{/snippet}
		</Popover.Trigger>
		<Popover.Content
			role="dialog"
			align="start"
			class="grid max-h-[min(32rem,calc(100dvh-2rem))] w-72 max-w-[calc(100vw-2rem)] gap-4 overflow-y-auto"
			aria-labelledby={`${uid}-title`}
		>
			<div class="grid gap-1">
				<h3 id={`${uid}-title`} class="text-sm font-medium">เลือกสีหลัก</h3>
				<p class="text-xs text-muted-foreground">เลือกสีสำเร็จรูป หรือปรับค่าสี RGB</p>
			</div>
			<div class="grid grid-cols-6 gap-2" role="group" aria-label="สีสำเร็จรูป">
				{#each presets as [label, hex] (hex)}
					<Button
						type="button"
						variant="outline"
						size="icon-sm"
						class="size-8 p-0"
						{disabled}
						aria-label={`${label} ${hex.toUpperCase()}`}
						aria-pressed={value.toLowerCase() === hex}
						onclick={() => !disabled && onTextChange(hex)}
					>
						<span
							class="grid size-6 place-items-center rounded-sm border border-foreground/30"
							style:background={hex}
						>
							{#if value.toLowerCase() === hex}<Check
									class="size-4 rounded-sm bg-background text-foreground"
								/>{/if}
						</span>
					</Button>
				{/each}
			</div>
			<div class="grid gap-3">
				{#each channelLabels as label, index (label)}
					<div class="grid gap-1">
						<div class="flex justify-between text-xs">
							<Label id={`${uid}-channel-${index}`}>{label}</Label><span class="tabular-nums"
								>{channels[index]}</span
							>
						</div>
						<Slider
							type="single"
							value={channels[index]}
							max={255}
							step={1}
							{disabled}
							onValueChange={(channel) => changeChannel(index, channel)}
							thumbProps={{ 'aria-labelledby': `${uid}-channel-${index}` }}
						/>
					</div>
				{/each}
			</div>
			<Popover.Close>
				{#snippet child({ props })}<Button {...props} variant="outline" size="sm">เสร็จสิ้น</Button
					>{/snippet}
			</Popover.Close>
		</Popover.Content>
	</Popover.Root>
	<Input
		{id}
		value={text}
		{disabled}
		aria-invalid={invalid}
		aria-describedby={describedby}
		oninput={(event) => !disabled && onTextChange(event.currentTarget.value)}
		spellcheck={false}
		maxlength={32}
		autocomplete="off"
		class="min-w-0 flex-1"
	/>
</div>
