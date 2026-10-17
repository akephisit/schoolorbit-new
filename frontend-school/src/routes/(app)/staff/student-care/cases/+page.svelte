<script lang="ts">
	import CareFilters from '#lib/components/student-care/CareFilters.svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';

	import { careIdentity, careSensitiveReadPermissions } from '#lib/features/student-care/access.js';
	import { careLink, careDate, caseLabels } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';
	import CareNavigation from '#lib/components/student-care/CareNavigation.svelte';
	import CarePager from '#lib/components/student-care/CarePager.svelte';

	import { careCases } from '#lib/api/student-care.js';
	let { data }: PageProps = $props();
</script>

<PageShell
	title="การดูแลรายกรณี"
	description="ทุกภาคเรียนในขอบเขตที่คุณรับผิดชอบ"
	backHref={careLink('', data.term)}
	>{#snippet actions()}{#if $can.has(PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL)}<Button
				href={careLink('/cases/new', data.term)}>เปิดการดูแล</Button
			>{/if}{/snippet}<CareNavigation current="cases" term={data.term} />{#key data.url}<CareFilters
			url={data.url}
			kind="cases"
		/>{/key}{#key $careIdentity + JSON.stringify(data.query)}<CareRegion
			initial={data.initial}
			permissions={careSensitiveReadPermissions}
			retry={(signal) => careCases(data.query, { signal })}
			label="การดูแลรายกรณี"
			>{#snippet children(items)}{#if !items.length}<PageState
						title="ยังไม่มีรายการ"
						description="รายการที่อยู่ในสิทธิ์ของคุณจะแสดงที่นี่"
					/>{:else}<ul class="divide-y rounded-xl border bg-card">
						{#each items as item (item.id)}<li
								class="flex flex-wrap items-center justify-between gap-3 p-4"
							>
								<div class="min-w-0 break-words space-y-1">
									<p class="font-medium">{item.title}</p>
									<p class="text-sm text-muted-foreground">
										{item.studentName} · {caseLabels[item.status]} · ติดตาม {careDate(
											item.nextFollowUp
										)}
									</p>
								</div>
								<Button variant="outline" href={careLink(`/cases/${item.id}`, data.term)}
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
