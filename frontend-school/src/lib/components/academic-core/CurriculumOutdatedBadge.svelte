<script lang="ts">
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Tooltip from '#lib/components/ui/tooltip/index.js';

	let { hasNewerCurrentVersion }: { hasNewerCurrentVersion: boolean } = $props();
	let open = $state(false);
</script>

{#if hasNewerCurrentVersion}
	<Tooltip.Provider>
		<Tooltip.Root bind:open disableCloseOnTriggerClick>
			<Tooltip.Trigger
				type="button"
				class="inline-flex rounded-full"
				onclick={() => (open = !open)}
			>
				<Badge
					variant="outline"
					class="border-amber-300 bg-amber-100 text-amber-900 dark:border-amber-800 dark:bg-amber-950 dark:text-amber-200"
					>เก่า</Badge
				>
			</Tooltip.Trigger>
			<Tooltip.Content>มีรายวิชารุ่นใหม่ที่เริ่มใช้แล้วในทะเบียน</Tooltip.Content>
		</Tooltip.Root>
	</Tooltip.Provider>
{/if}
