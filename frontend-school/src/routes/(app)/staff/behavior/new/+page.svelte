<script lang="ts">
	import { ClipboardPen } from '@lucide/svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { behaviorItems, behaviorStudents } from '#lib/api/behavior.js';
	import { behaviorIdentity, behaviorCreatePermissions } from '#lib/features/behavior/access.js';
	import BehaviorRegion from '#lib/components/behavior/BehaviorRegion.svelte';
	import BehaviorRecordForm from '#lib/components/behavior/BehaviorRecordForm.svelte';
	let { data }: PageProps = $props();
</script>

<PageShell
	icon={ClipboardPen}
	title="บันทึกพฤติกรรม"
	description="เลือกรายการและนักเรียนรายคนหรือกลุ่มผสมข้ามห้อง"
	backHref={`/staff/behavior?academicTermId=${data.term ?? ''}`}
	>{#if data.initial && data.term}{#key $behaviorIdentity + data.term}<BehaviorRegion
				initial={data.initial}
				permissions={behaviorCreatePermissions}
				variant="form"
				retry={async (signal) => {
					const [items, roster] = await Promise.all([
						behaviorItems({ date: data.date }, { signal }),
						behaviorStudents({ academicTermId: data.term!, date: data.date, limit: 50 }, { signal })
					]);
					return { items, roster };
				}}
				>{#snippet children(result)}<BehaviorRecordForm
						term={data.term!}
						date={data.date}
						initialItems={result.items}
						initialRoster={result.roster}
					/>{/snippet}</BehaviorRegion
			>{/key}{:else}<PageState title="เลือกภาคเรียน" />{/if}</PageShell
>
