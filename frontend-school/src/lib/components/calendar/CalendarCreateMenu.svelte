<script lang="ts">
	import { Plus, CalendarPlus, ClipboardPlus } from '@lucide/svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';

	let {
		canCreate,
		canRequest,
		createDisabled = false,
		label = 'เพิ่มรายการปฏิทิน',
		compact = false,
		oncreate,
		onrequest
	}: {
		canCreate: boolean;
		canRequest: boolean;
		createDisabled?: boolean;
		label?: string;
		compact?: boolean;
		oncreate: () => void;
		onrequest: () => void;
	} = $props();
</script>

{#if canCreate || canRequest}
	<DropdownMenu.Root>
		<DropdownMenu.Trigger>
			{#snippet child({ props })}
				<Button
					{...props}
					size={compact ? 'icon-sm' : 'icon'}
					variant={compact ? 'outline' : 'default'}
					aria-label={label}
					title={label}
				>
					<Plus class="size-4" />
				</Button>
			{/snippet}
		</DropdownMenu.Trigger>
		<DropdownMenu.Content align="end" class="w-60">
			{#if canCreate}
				<DropdownMenu.Item disabled={createDisabled} onSelect={oncreate}>
					<CalendarPlus class="size-4" />เพิ่มกิจกรรม
				</DropdownMenu.Item>
			{/if}
			{#if canRequest}
				<DropdownMenu.Item onSelect={onrequest}>
					<ClipboardPlus class="size-4" />คำร้องขอเพิ่มกิจกรรม
				</DropdownMenu.Item>
			{/if}
		</DropdownMenu.Content>
	</DropdownMenu.Root>
{/if}
