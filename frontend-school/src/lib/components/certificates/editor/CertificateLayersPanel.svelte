<script lang="ts">
	import type { CertificateElement, LayerDirection } from '$lib/certificates/editor-state';
	import { Button } from '$lib/components/ui/button';
	import { ArrowDown, ArrowUp, Braces, Image as ImageIcon, Layers3, QrCode } from 'lucide-svelte';

	let {
		elements,
		selectedIds,
		disabled = false,
		onselect,
		onreorder
	}: {
		elements: CertificateElement[];
		selectedIds: string[];
		disabled?: boolean;
		onselect: (elementId: string, additive: boolean) => void;
		onreorder: (elementId: string, direction: LayerDirection) => void;
	} = $props();

	const topFirst = $derived([...elements].reverse());

	function layerName(element: CertificateElement): string {
		if (element.type === 'qr') return 'QR Code ตรวจสอบ';
		if (element.type === 'image') return 'รูปประกอบ';
		const value = element.content.replace(/\s+/gu, ' ').trim();
		return value || 'ข้อความว่าง';
	}
</script>

<section class="space-y-3" aria-labelledby="certificate-layers-title">
	<div class="flex items-center justify-between gap-3">
		<div>
			<h2 id="certificate-layers-title" class="flex items-center gap-2 text-sm font-semibold">
				<Layers3 class="size-4 text-primary" /> ลำดับชั้น
			</h2>
			<p class="mt-0.5 text-[0.7rem] text-muted-foreground">รายการบนสุดจะแสดงทับรายการด้านล่าง</p>
		</div>
		<span class="rounded-full bg-muted px-2 py-0.5 text-[0.7rem] tabular-nums">
			{elements.length}
		</span>
	</div>

	{#if topFirst.length === 0}
		<div
			class="rounded-lg border border-dashed px-3 py-6 text-center text-xs text-muted-foreground"
		>
			ยังไม่มีองค์ประกอบบนหน้า
		</div>
	{:else}
		<div class="space-y-1.5" role="list" aria-label="ลำดับองค์ประกอบ">
			{#each topFirst as element (element.id)}
				{@const selected = selectedIds.includes(element.id)}
				<div
					class={[
						'group flex items-center gap-1 rounded-lg border p-1 transition-colors',
						selected ? 'border-primary/40 bg-primary/8' : 'border-transparent hover:bg-muted/60'
					]}
					role="listitem"
				>
					<button
						type="button"
						class="flex min-w-0 flex-1 items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
						{disabled}
						onclick={(event) => onselect(element.id, event.shiftKey)}
						aria-pressed={selected}
					>
						{#if element.type === 'text'}
							<Braces class="size-3.5 shrink-0" />
						{:else if element.type === 'image'}
							<ImageIcon class="size-3.5 shrink-0" />
						{:else}
							<QrCode class="size-3.5 shrink-0" />
						{/if}
						<span class="truncate">{layerName(element)}</span>
					</button>
					<div class="flex shrink-0 opacity-60 transition-opacity group-hover:opacity-100">
						<Button
							type="button"
							size="icon-sm"
							variant="ghost"
							{disabled}
							onclick={() => onreorder(element.id, 'forward')}
							aria-label={`เลื่อน ${layerName(element)} ขึ้นหนึ่งชั้น`}
						>
							<ArrowUp class="size-3.5" />
						</Button>
						<Button
							type="button"
							size="icon-sm"
							variant="ghost"
							{disabled}
							onclick={() => onreorder(element.id, 'backward')}
							aria-label={`เลื่อน ${layerName(element)} ลงหนึ่งชั้น`}
						>
							<ArrowDown class="size-3.5" />
						</Button>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</section>
