<script lang="ts">
	import { SvelteURLSearchParams } from 'svelte/reactivity';
	import type { PageProps } from './$types';
	import { goto } from '$app/navigation';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import {
		behaviorIdentity,
		behaviorReadPermissions,
		behaviorCreatePermissions
	} from '#lib/features/behavior/access.js';
	import { behaviorOverview, behaviorEntries, behaviorSummary } from '#lib/api/behavior.js';
	import { sideLabels } from '#lib/features/behavior/presentation.js';
	import BehaviorRegion from '#lib/components/behavior/BehaviorRegion.svelte';
	import BehaviorLog from '#lib/components/behavior/BehaviorLog.svelte';
	import BehaviorSummary from '#lib/components/behavior/BehaviorSummary.svelte';
	let { data }: PageProps = $props();
	let summaryRevision = $state(0);
	function destination(suffix: string) {
		const p = new SvelteURLSearchParams();
		if (data.term) p.set('academicTermId', data.term);
		return `/staff/behavior${suffix}?${p}`;
	}
	function student(id: string) {
		const p = new SvelteURLSearchParams({ academicTermId: data.term ?? '', studentId: id });
		void goto(`/staff/behavior?${p}`, { reset: false });
	}
</script>

<PageShell title={data.title} description="คะแนนความดี คะแนนที่ถูกหัก และประวัติรายบุคคล"
	>{#snippet actions()}{#if $can.hasAny(...behaviorCreatePermissions)}<Button
				class="min-h-[44px] min-w-[44px]"
				href={destination('/new')}>บันทึกพฤติกรรม</Button
			>{/if}{#if $can.has(PERMISSIONS.BEHAVIOR_MANAGE_SCHOOL)}<Button
				class="min-h-[44px] min-w-[44px]"
				href={destination('/settings')}
				variant="outline">ตั้งค่า</Button
			>{/if}{#if $can.has(PERMISSIONS.BEHAVIOR_CLEANUP_EXECUTE_SCHOOL)}<Button
				class="min-h-[44px] min-w-[44px]"
				href={destination('/cleanup')}
				variant="outline">รีเซ็ต / ล้างข้อมูล</Button
			>{/if}{/snippet}
	{#key $behaviorIdentity + data.term}<BehaviorRegion
			initial={data.overview}
			permissions={behaviorReadPermissions}
			retry={(signal) => behaviorOverview({ signal })}
			variant="detail"
			>{#snippet children(overview)}<section class="rounded-md border bg-primary/5 p-4">
					<h2 class="font-semibold">รอบคะแนน</h2>
					<p class="text-sm text-muted-foreground">เริ่มใช้ระบบ {overview.activationDate}</p>
					{#each overview.cycles.filter((c) => c.state === 'current') as cycle (cycle.id)}<p>
							{sideLabels[cycle.side]} · {cycle.label} · เริ่ม {cycle.startsOn}
						</p>{/each}
				</section>{/snippet}</BehaviorRegion
		>{/key}
	{#if data.summary && data.studentId}{#key $behaviorIdentity + data.studentId + data.term}<BehaviorRegion
				initial={data.summary}
				permissions={behaviorReadPermissions}
				retry={(signal) => behaviorSummary(data.studentId!, {}, { signal })}
				variant="detail"
				>{#snippet children(summary)}<BehaviorSummary
						initial={summary}
						refreshRevision={summaryRevision}
					/>{/snippet}</BehaviorRegion
			>{/key}{:else}<p class="text-sm text-muted-foreground">
			เลือกชื่อนักเรียนจากรายการด้านล่างเพื่อดูคะแนนปัจจุบัน ประวัติ และสะสม
		</p>{/if}
	{#if data.entries}{#key $behaviorIdentity + data.term}<BehaviorRegion
				initial={data.entries}
				permissions={behaviorReadPermissions}
				retry={(signal) => behaviorEntries(data.query, { signal })}
				>{#snippet children(entries)}<BehaviorLog
						initial={entries}
						query={data.query}
						onStudent={student}
						onChanged={() => summaryRevision++}
					/>{/snippet}</BehaviorRegion
			>{/key}{:else}<PageState title="เลือกภาคเรียน" description="เลือกภาคเรียนจากแถบด้านบน" />{/if}
</PageShell>
