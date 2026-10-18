<script lang="ts">
	import CareFilters from '#lib/components/student-care/CareFilters.svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { can } from '#lib/stores/permissions.js';

	import {
		careIdentity,
		careRoutineReadPermissions,
		careManagePermissions
	} from '#lib/features/student-care/access.js';
	import { careLink, careDate, visitLabels } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';
	import CareNavigation from '#lib/components/student-care/CareNavigation.svelte';
	import CarePager from '#lib/components/student-care/CarePager.svelte';

	import { careVisits } from '#lib/api/student-care.js';
	let { data }: PageProps = $props();
</script>

<PageShell
	title="เยี่ยมบ้าน"
	description="ทุกภาคเรียนในขอบเขตที่คุณรับผิดชอบ"
	backHref={careLink('', data.term)}
	>{#snippet actions()}{#if $can.hasAny(...careManagePermissions)}<Button
				href={careLink('/visits/new', data.term)}>วางแผนเยี่ยมบ้าน</Button
			>{/if}{/snippet}<CareNavigation
		current="visits"
		term={data.term}
	/>{#key data.url}<CareFilters
			url={data.url}
			kind="visits"
		/>{/key}{#key $careIdentity + JSON.stringify(data.query)}<CareRegion
			initial={data.initial}
			permissions={careRoutineReadPermissions}
			retry={(signal) => careVisits(data.query, { signal })}
			label="เยี่ยมบ้าน"
			>{#snippet children(items)}{#if !items.length}<PageState
						title="ยังไม่มีรายการ"
						description="รายการที่อยู่ในสิทธิ์ของคุณจะแสดงที่นี่"
					/>{:else}<ul class="divide-y rounded-xl border bg-card">
						{#each items as item (item.id)}<li
								class="flex flex-wrap items-center justify-between gap-3 p-4"
							>
								<div class="min-w-0 break-words space-y-1">
									<p class="font-medium">{item.studentName}</p>
									<p class="text-sm text-muted-foreground">
										{visitLabels[item.status]} · นัดหมาย {careDate(item.plannedDate)}
									</p>
								</div>
								<Button variant="outline" href={careLink(`/visits/${item.id}`, data.term)}
									>ดูรายละเอียด</Button
								>
							</li>{/each}
					</ul>{/if}<CarePager
					url={data.url}
					lastId={items.at(-1)?.id}
					count={items.length}
				/>{/snippet}</CareRegion
		>{/key}</PageShell
>
