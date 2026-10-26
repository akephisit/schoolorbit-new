<script lang="ts">
	import { Badge } from '#lib/components/ui/badge/index.js';
	import * as Tooltip from '#lib/components/ui/tooltip/index.js';

	let {
		hasNewerCurrentVersion,
		resourceKind
	}: { hasNewerCurrentVersion: boolean; resourceKind: 'course' | 'activity' } = $props();
	let open = $state(false);
</script>

{#if hasNewerCurrentVersion}
	<Tooltip.Provider>
		<Tooltip.Root bind:open disableCloseOnTriggerClick>
			<Tooltip.Trigger type="button" class="inline-flex rounded-full" onclick={() => (open = true)}>
				<Badge variant="outline" class="border-warning/30 bg-warning/10 text-warning">เก่า</Badge>
			</Tooltip.Trigger>
			<Tooltip.Content role="tooltip"
				>{resourceKind === 'activity'
					? 'มีกิจกรรมรุ่นใหม่ที่เริ่มใช้แล้วในทะเบียน'
					: 'มีรายวิชารุ่นใหม่ที่เริ่มใช้แล้วในทะเบียน'}</Tooltip.Content
			>
		</Tooltip.Root>
	</Tooltip.Provider>
{/if}
