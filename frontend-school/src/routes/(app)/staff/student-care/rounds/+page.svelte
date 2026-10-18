<script lang="ts">
	import { ClipboardList } from '@lucide/svelte';
	import CareFilters from '#lib/components/student-care/CareFilters.svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import CareStatus from '#lib/components/student-care/CareStatus.svelte';
	import { PageState } from '#lib/components/app-state/index.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { careIdentity, careReadPermissions } from '#lib/features/student-care/access.js';
	import { careLink, careDate } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';
	import CareNavigation from '#lib/components/student-care/CareNavigation.svelte';
	import CarePager from '#lib/components/student-care/CarePager.svelte';

	import { careRounds } from '#lib/api/student-care.js';
	let { data }: PageProps = $props();
</script>

<PageShell
	backLabel="กลับภาพรวมการดูแล"
	icon={ClipboardList}
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
						title={data.query.afterId || data.query.status || data.query.afterId
							? 'ไม่พบรอบประเมินในรายการนี้'
							: 'ยังไม่มีรอบประเมิน'}
						description={data.query.afterId || data.query.status
							? 'ลองล้างตัวกรองหรือกลับหน้าแรกเพื่อดูรายการอื่นในขอบเขตของคุณ'
							: 'เริ่มสร้างรอบเพื่อเลือกรุ่นแบบประเมินและมอบหมายผู้ตอบ ผู้ไม่มีสิทธิ์สร้างรอบให้ติดต่อผู้รับผิดชอบ'}
					/>{:else}<ul class="divide-y rounded-xl border bg-card">
						{#each items as item (item.id)}<li
								class="flex flex-wrap items-center justify-between gap-3 p-4"
							>
								<div class="min-w-0 break-words space-y-1">
									<p class="font-medium">{item.name}</p>
									<CareStatus status={{ kind: 'round', value: item.status }} />
									<p class="text-sm text-muted-foreground">
										กำหนดส่ง {careDate(item.dueDate)}
									</p>
								</div>
								<Button variant="outline" href={careLink(`/rounds/${item.id}`, data.term)}
									>ดูรอบประเมิน</Button
								>
							</li>{/each}
					</ul>{/if}<CarePager
					url={data.url}
					lastId={items.at(-1)?.id}
					count={items.length}
				/>{/snippet}</CareRegion
		>{/key}</PageShell
>
