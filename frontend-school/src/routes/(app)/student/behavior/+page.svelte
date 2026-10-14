<script lang="ts">
	import { Award } from '@lucide/svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { behaviorIdentity } from '#lib/features/behavior/access.js';
	import { loadOwnBehavior } from '#lib/features/behavior/own-route.js';
	import { behaviorSummary, behaviorEntries } from '#lib/api/behavior.js';
	import BehaviorRegion from '#lib/components/behavior/BehaviorRegion.svelte';
	import BehaviorSummary from '#lib/components/behavior/BehaviorSummary.svelte';
	import BehaviorLog from '#lib/components/behavior/BehaviorLog.svelte';
	let { data }: PageProps = $props();
	const permissions = [PERMISSIONS.BEHAVIOR_READ_OWN];
</script>

<PageShell
	title="พฤติกรรมและคะแนน"
	icon={Award}
	description="คะแนนปัจจุบัน ยอดสะสม และประวัติพฤติกรรม"
	>{#key $behaviorIdentity + data.url}<BehaviorRegion
			initial={data.initial}
			{permissions}
			variant="detail"
			retry={(signal) => loadOwnBehavior(fetch, new URL(data.url), 'student', signal)}
			>{#snippet children(context)}{#if context}<nav
						class="flex flex-wrap gap-2 rounded-xl border bg-card p-3 sm:p-4"
						aria-label="เลือกภาคเรียน"
					>
						{#each context.terms as term (term.id)}<Button
								class="min-h-[44px] min-w-[44px]"
								variant={term.id === context.termId ? 'default' : 'outline'}
								href={`/student/behavior?academicTermId=${term.id}${context.studentId ? '&studentId=' + context.studentId : ''}`}
								>ภาคเรียน {term.name}</Button
							>{/each}
					</nav>
					{#if context.children.length}<nav
							class="flex flex-wrap gap-2 rounded-xl border bg-card p-3 sm:p-4"
							aria-label="เลือกนักเรียน"
						>
							{#each context.children as child (child.id)}<Button
									class="min-h-[44px] min-w-[44px]"
									variant={child.id === context.studentId ? 'default' : 'outline'}
									href={`/student/behavior?academicTermId=${context.termId ?? ''}&studentId=${child.id}`}
									>{child.first_name} {child.last_name}</Button
								>{/each}
						</nav>{/if}{#if context.summary && context.studentId}<BehaviorRegion
							initial={context.summary}
							{permissions}
							variant="detail"
							retry={(signal) => behaviorSummary(context.studentId!, {}, { signal })}
							>{#snippet children(summary)}<BehaviorSummary
									initial={summary}
								/>{/snippet}</BehaviorRegion
						>{/if}{#if context.entries}<BehaviorRegion
							initial={context.entries}
							{permissions}
							retry={(signal) => behaviorEntries(context.query, { signal })}
							>{#snippet children(entries)}<BehaviorLog
									initial={entries}
									query={context.query}
								/>{/snippet}</BehaviorRegion
						>{:else}<PageState title="ไม่มีข้อมูลนักเรียนในภาคเรียนนี้" />{/if}{:else}<PageState
						title="ไม่มีข้อมูลในขอบเขตของบัญชี"
					/>{/if}{/snippet}</BehaviorRegion
		>{/key}</PageShell
>
