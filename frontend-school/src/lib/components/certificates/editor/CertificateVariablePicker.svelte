<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Braces, CornerDownLeft } from 'lucide-svelte';

	let {
		variables,
		disabled = false,
		oninsert
	}: {
		variables: string[];
		disabled?: boolean;
		oninsert: (token: string) => void;
	} = $props();

	let selectedVariable = $state('');

	function insertSelected() {
		if (!selectedVariable || disabled) return;
		oninsert(`{${selectedVariable}}`);
	}
</script>

<div class="space-y-2">
	<label for="certificate-variable-picker" class="text-xs font-medium text-muted-foreground">
		ตัวแปรจากรายชื่อ
	</label>
	<div class="flex gap-2">
		<div class="relative min-w-0 flex-1">
			<Braces
				class="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
			/>
			<select
				id="certificate-variable-picker"
				class="h-9 w-full rounded-md border bg-background pr-8 pl-8 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
				bind:value={selectedVariable}
				{disabled}
			>
				<option value="">เลือกตัวแปร</option>
				{#each variables as variable (variable)}
					<option value={variable}>{variable}</option>
				{/each}
			</select>
		</div>
		<Button
			type="button"
			size="icon-sm"
			variant="outline"
			disabled={disabled || !selectedVariable}
			onclick={insertSelected}
			aria-label="แทรกตัวแปรในข้อความ"
		>
			<CornerDownLeft class="size-4" />
		</Button>
	</div>
</div>
