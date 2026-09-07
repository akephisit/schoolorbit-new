<script lang="ts">
	import { LoadingButton } from '$lib/components/app-state';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { CircleAlert } from 'lucide-svelte';

	let {
		open,
		title,
		description,
		blankCount,
		busy = false,
		onopenchange,
		onconfirm
	}: {
		open: boolean;
		title: string;
		description: string;
		blankCount: number;
		busy?: boolean;
		onopenchange: (open: boolean) => void;
		onconfirm: () => void;
	} = $props();
</script>

<Dialog.Root {open} onOpenChange={onopenchange}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>{title}</Dialog.Title>
			<Dialog.Description>{description}</Dialog.Description>
		</Dialog.Header>
		{#if blankCount > 0}
			<div
				class="flex gap-3 rounded-lg border border-amber-200 bg-amber-50 p-3 text-sm text-amber-900"
			>
				<CircleAlert class="mt-0.5 size-4 shrink-0" />
				<p>
					มีช่องว่าง {blankCount.toLocaleString('th-TH')} ช่อง ระบบยังเก็บเป็นช่องว่าง แต่จะนับเป็นศูนย์เมื่อนำไปคำนวณผลการเรียน
				</p>
			</div>
		{/if}
		<Dialog.Footer>
			<Button variant="outline" disabled={busy} onclick={() => onopenchange(false)}
				>กลับไปตรวจ</Button
			>
			<LoadingButton loading={busy} onclick={onconfirm}>ยืนยัน</LoadingButton>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
