<script lang="ts">
	import { HeartHandshake } from '@lucide/svelte';
	import CareFilters from '#lib/components/student-care/CareFilters.svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import CareStatus from '#lib/components/student-care/CareStatus.svelte';
	import { PageState } from '#lib/components/app-state/index.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { can } from '#lib/stores/permissions.js';

	import { careIdentity, careSensitiveReadPermissions } from '#lib/features/student-care/access.js';
	import { careLink, careDate } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';
	import CareNavigation from '#lib/components/student-care/CareNavigation.svelte';
	import CarePager from '#lib/components/student-care/CarePager.svelte';

	import { careCases } from '#lib/api/student-care.js';
	let { data }: PageProps = $props();
</script>

<PageShell
	backLabel="กลับภาพรวมการดูแล"
	icon={HeartHandshake}
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
						title={data.query.afterId || data.query.status || data.query.followUpBefore
							? 'ไม่พบการดูแลรายกรณีในรายการนี้'
							: 'ยังไม่มีการดูแลรายกรณี'}
						description={data.query.afterId || data.query.status || data.query.followUpBefore
							? 'ลองล้างตัวกรองหรือกลับหน้าแรกเพื่อดูรายการอื่นในขอบเขตของคุณ'
							: 'ผู้มีสิทธิ์สามารถเปิดการดูแลจากแบบประเมินหรือบันทึกเยี่ยมบ้านที่รับผิดชอบ'}
					/>{:else}<ul class="divide-y rounded-xl border bg-card">
						{#each items as item (item.id)}<li
								class="flex flex-wrap items-center justify-between gap-3 p-4"
							>
								<div class="min-w-0 break-words space-y-1">
									<p class="font-medium">{item.title}</p>
									<CareStatus status={{ kind: 'case', value: item.status }} />
									<p class="text-sm text-muted-foreground">
										{item.studentName} · ติดตาม {careDate(item.nextFollowUp)}
									</p>
								</div>
								<Button variant="outline" href={careLink(`/cases/${item.id}`, data.term)}
									>ดูการดูแล</Button
								>
							</li>{/each}
					</ul>{/if}<CarePager
					url={data.url}
					lastId={items.at(-1)?.id}
					count={items.length}
				/>{/snippet}</CareRegion
		>{/key}</PageShell
>
