<script lang="ts">
	import { formatBehaviorDate } from '#lib/features/behavior/presentation.js';
	import { Award, Plus, Settings2, RotateCcw } from '@lucide/svelte';
	import * as Card from '#lib/components/ui/card/index.js';
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

<PageShell
	icon={Award}
	title={data.title}
	description="คะแนนความดี คะแนนที่ถูกหัก และประวัติรายบุคคล"
	>{#snippet actions()}{#if $can.hasAny(...behaviorCreatePermissions)}<Button
				href={destination('/new')}><Plus class="size-4" aria-hidden="true" />บันทึกพฤติกรรม</Button
			>{/if}{#if $can.has(PERMISSIONS.BEHAVIOR_MANAGE_SCHOOL)}<Button
				href={destination('/settings')}
				variant="outline"><Settings2 class="size-4" aria-hidden="true" />ตั้งค่า</Button
			>{/if}{#if $can.has(PERMISSIONS.BEHAVIOR_CLEANUP_EXECUTE_SCHOOL)}<Button
				href={destination('/cleanup')}
				variant="ghost"><RotateCcw class="size-4" aria-hidden="true" />รีเซ็ต / ล้างข้อมูล</Button
			>{/if}{/snippet}
	{#key $behaviorIdentity + data.term}<BehaviorRegion
			initial={data.overview}
			permissions={behaviorReadPermissions}
			retry={(signal) => behaviorOverview({ signal })}
			variant="detail"
			>{#snippet children(overview)}<Card.Root
					class="gap-0 border-primary/20 bg-gradient-to-br from-primary/10 via-card to-card py-0"
					><Card.Content class="p-4 sm:p-5">
						<div class="flex flex-wrap items-start justify-between gap-3">
							<div>
								<h2 class="font-semibold">รอบคะแนน</h2>
								<p class="mt-1 text-sm text-muted-foreground">
									เริ่มใช้ระบบ {formatBehaviorDate(overview.activationDate)}
								</p>
							</div>
							<Award class="size-5 text-primary" aria-hidden="true" />
						</div>
						<dl class="mt-4 grid gap-3 sm:grid-cols-2">
							{#each overview.cycles.filter((cycle) => cycle.state === 'current') as cycle (cycle.id)}<div
									class="space-y-1 border-t border-primary/15 pt-3"
								>
									<dt class="text-sm font-medium">{sideLabels[cycle.side]}</dt>
									<dd class="mt-1 text-sm text-muted-foreground">
										{cycle.label} · เริ่ม {formatBehaviorDate(cycle.startsOn)}
									</dd>
								</div>{/each}
						</dl>
					</Card.Content></Card.Root
				>{/snippet}</BehaviorRegion
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
			>{/key}{:else}<p
			class="rounded-xl border border-dashed bg-card p-4 text-sm text-muted-foreground"
		>
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
