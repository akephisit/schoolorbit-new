<script lang="ts">
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Settings2 } from '@lucide/svelte';

	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { behaviorIdentity } from '#lib/features/behavior/access.js';
	import { behaviorItems } from '#lib/api/behavior.js';
	import BehaviorRegion from '#lib/components/behavior/BehaviorRegion.svelte';
	import BehaviorSettingsNavigation from '#lib/components/behavior/BehaviorSettingsNavigation.svelte';
	import BehaviorItemsEditor from '#lib/components/behavior/BehaviorItemsEditor.svelte';
	let { data }: PageProps = $props();
	const permissions = [PERMISSIONS.BEHAVIOR_MANAGE_SCHOOL];
</script>

<PageShell
	title="รายการพฤติกรรม"
	description="กำหนดรายการความดีและความผิด พร้อมคะแนนและวันเริ่มใช้"
	icon={Settings2}
	backHref={`/staff/behavior?academicTermId=${data.term ?? ''}`}
>
	<BehaviorSettingsNavigation term={data.term} active="items" />
	<div class="max-w-6xl">
		{#key $behaviorIdentity + data.term}<BehaviorRegion
				initial={data.items}
				{permissions}
				retry={(signal) => behaviorItems({ date: data.date, includeDisabled: true }, { signal })}
				>{#snippet children(items)}<BehaviorItemsEditor initial={items} />{/snippet}</BehaviorRegion
			>{/key}
	</div>
</PageShell>
