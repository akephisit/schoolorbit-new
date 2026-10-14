<script lang="ts">
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Settings2 } from '@lucide/svelte';

	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { behaviorIdentity } from '#lib/features/behavior/access.js';
	import { behaviorSettings } from '#lib/api/behavior.js';
	import BehaviorRegion from '#lib/components/behavior/BehaviorRegion.svelte';
	import BehaviorSettingsNavigation from '#lib/components/behavior/BehaviorSettingsNavigation.svelte';
	import BehaviorSettingsConfig from '#lib/components/behavior/BehaviorSettingsConfig.svelte';
	let { data }: PageProps = $props();
	const permissions = [PERMISSIONS.BEHAVIOR_MANAGE_SCHOOL];
</script>

<PageShell
	title="นโยบายคะแนนและรอบรีเซ็ต"
	description="กำหนดการคิดคะแนนอัตโนมัติ เพดานรายวัน และรอบรีเซ็ตของแต่ละฝั่ง"
	icon={Settings2}
	backHref={`/staff/behavior?academicTermId=${data.term ?? ''}`}
>
	<BehaviorSettingsNavigation term={data.term} active="policy" />
	<div class="max-w-6xl">
		{#key $behaviorIdentity + data.term}<BehaviorRegion
				initial={data.settings}
				{permissions}
				variant="form"
				retry={(signal) => behaviorSettings({ signal })}
				>{#snippet children(settings)}<BehaviorSettingsConfig
						initial={settings}
					/>{/snippet}</BehaviorRegion
			>{/key}
	</div>
</PageShell>
