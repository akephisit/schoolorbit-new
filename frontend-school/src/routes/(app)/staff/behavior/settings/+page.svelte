<script lang="ts">
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { behaviorIdentity } from '#lib/features/behavior/access.js';
	import {
		behaviorSettings,
		behaviorItems,
		behaviorRules,
		behaviorOptions
	} from '#lib/api/behavior.js';
	import BehaviorRegion from '#lib/components/behavior/BehaviorRegion.svelte';
	import BehaviorSettingsConfig from '#lib/components/behavior/BehaviorSettingsConfig.svelte';
	import BehaviorItemsEditor from '#lib/components/behavior/BehaviorItemsEditor.svelte';
	import BehaviorRulesEditor from '#lib/components/behavior/BehaviorRulesEditor.svelte';
	let { data }: PageProps = $props();
	const permissions = [PERMISSIONS.BEHAVIOR_MANAGE_SCHOOL];
</script>

<PageShell
	title="ตั้งค่าพฤติกรรมและคะแนน"
	backHref={`/staff/behavior?academicTermId=${data.term ?? ''}`}
>
	{#key $behaviorIdentity + data.term}<BehaviorRegion
			initial={data.settings}
			{permissions}
			variant="form"
			retry={(signal) => behaviorSettings({ signal })}
			>{#snippet children(settings)}<BehaviorSettingsConfig
					initial={settings}
				/>{/snippet}</BehaviorRegion
		><BehaviorRegion
			initial={data.items}
			{permissions}
			retry={(signal) => behaviorItems({ date: data.date, includeDisabled: true }, { signal })}
			>{#snippet children(items)}<BehaviorItemsEditor initial={items} />{/snippet}</BehaviorRegion
		>{#if data.rules}<BehaviorRegion
				initial={data.rules}
				{permissions}
				retry={async (signal) => {
					const [rules, options] = await Promise.all([
						behaviorRules({ signal }),
						behaviorOptions({ academicTermId: data.term!, date: data.date }, { signal })
					]);
					return { rules, options };
				}}
				>{#snippet children(result)}<BehaviorRulesEditor
						initial={result}
					/>{/snippet}</BehaviorRegion
			>{:else}<PageState title="เลือกภาคเรียนเพื่อตั้งเกณฑ์ระดับชั้น" />{/if}{/key}</PageShell
>
