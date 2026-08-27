<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import * as Select from '$lib/components/ui/select';
	import { Braces, CornerDownLeft } from 'lucide-svelte';

	const NO_VARIABLE_VALUE = '__no_variable__';

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
			<Select.Root
				type="single"
				value={selectedVariable || NO_VARIABLE_VALUE}
				{disabled}
				onValueChange={(value) => (selectedVariable = value === NO_VARIABLE_VALUE ? '' : value)}
			>
				<Select.Trigger id="certificate-variable-picker" class="w-full pl-8 text-xs">
					{selectedVariable || 'เลือกตัวแปร'}
				</Select.Trigger>
				<Select.Content>
					<Select.Item value={NO_VARIABLE_VALUE}>เลือกตัวแปร</Select.Item>
					{#each variables as variable (variable)}
						<Select.Item value={variable}>{variable}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
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
