<script lang="ts">
	import CareFilters from '#lib/components/student-care/CareFilters.svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { careIdentity, careReadPermissions } from '#lib/features/student-care/access.js';
	import { careLink, careDate, roundLabels } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';
	import CareNavigation from '#lib/components/student-care/CareNavigation.svelte';
	import CarePager from '#lib/components/student-care/CarePager.svelte';

	import { careRounds } from '#lib/api/student-care.js';
	let { data }: PageProps = $props();
</script>

<PageShell
	title="รอบประเมิน"
	description="ทุกภาคเรียนในขอบเขตที่คุณรับผิดชอบ"
	backHref={careLink('', data.term)}
	>{#snippet actions()}{#if $can.hasAny(...[PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL])}<Button
				href={careLink('/rounds/new', data.term)}>สร้างรอบประเมิน</Button
			>{/if}{/snippet}<CareNavigation
		current="rounds"
		term={data.term}
	/>{#key data.url}<CareFilters
			url={data.url}
			kind="rounds"
		/>{/key}{#key $careIdentity + JSON.stringify(data.query)}<CareRegion
			initial={data.initial}
			permissions={careReadPermissions}
			retry={(signal) => careRounds(data.query, { signal })}
			label="รอบประเมิน"
			>{#snippet children(items)}{#if !items.length}<PageState
						title="ยังไม่มีรายการ"
						description="รายการที่อยู่ในสิทธิ์ของคุณจะแสดงที่นี่"
					/>{:else}<ul class="divide-y rounded-xl border bg-card">
						{#each items as item (item.id)}<li
								class="flex flex-wrap items-center justify-between gap-3 p-4"
							>
								<div class="min-w-0 break-words space-y-1">
									<p class="font-medium">{item.name}</p>
									<p class="text-sm text-muted-foreground">
										{roundLabels[item.status]} · กำหนดส่ง {careDate(item.dueDate)}
									</p>
								</div>
								<Button variant="outline" href={careLink(`/rounds/${item.id}`, data.term)}
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
