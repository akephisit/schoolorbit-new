<script lang="ts">
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Settings2 } from '@lucide/svelte';
	import { PageState } from '#lib/components/app-state/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { behaviorIdentity } from '#lib/features/behavior/access.js';
	import { behaviorRules, behaviorOptions } from '#lib/api/behavior.js';
	import BehaviorRegion from '#lib/components/behavior/BehaviorRegion.svelte';
	import BehaviorSettingsNavigation from '#lib/components/behavior/BehaviorSettingsNavigation.svelte';
	import BehaviorRulesEditor from '#lib/components/behavior/BehaviorRulesEditor.svelte';
	let { data }: PageProps = $props();
	const permissions = [PERMISSIONS.BEHAVIOR_MANAGE_SCHOOL];
</script>

<PageShell
	title="เกณฑ์คะแนนจากเช็กชื่อ"
	description="กำหนดคะแนนของแต่ละสถานะ แยกตามประเภทเช็กชื่อและระดับชั้น"
	icon={Settings2}
	backHref={`/staff/behavior?academicTermId=${data.term ?? ''}`}
>
	<BehaviorSettingsNavigation term={data.term} active="attendance" />
	<div class="max-w-6xl">
		{#key $behaviorIdentity + data.term}{#if data.rules}<BehaviorRegion
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
				>{:else}<PageState title="เลือกภาคเรียนเพื่อตั้งเกณฑ์ระดับชั้น" />{/if}{/key}
	</div>
</PageShell>
