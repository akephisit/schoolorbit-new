<script lang="ts">
	import { UserRound, ClipboardList, House, HeartHandshake } from '@lucide/svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';

	import { can } from '#lib/stores/permissions.js';

	import {
		careIdentity,
		careReadPermissions,
		careSensitiveReadPermissions
	} from '#lib/features/student-care/access.js';
	import {
		careLink,
		careDate,
		visitLabels,
		caseLabels
	} from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';

	import CareSection from '#lib/components/student-care/CareSection.svelte';
	import { careAssignments, careVisits, careCases } from '#lib/api/student-care.js';
	import { careRoutineReadPermissions } from '#lib/features/student-care/access.js';
	import CareAssignmentList from '#lib/components/student-care/CareAssignmentList.svelte';
	let { data }: PageProps = $props();
</script>

<PageShell
	backLabel="กลับภาพรวมการดูแล"
	icon={UserRound}
	title="ประวัติการดูแลรายบุคคล"
	backHref={careLink('', data.term)}
	>{#key $careIdentity + data.id}<CareSection
			title="แบบประเมิน"
			icon={ClipboardList}
			description="แสดงไม่เกิน 50 รายการแรก ทุกภาคเรียนในขอบเขตของคุณ"
			><CareRegion
				initial={data.assignments}
				permissions={careReadPermissions}
				retry={(signal) => careAssignments(data.query, { signal })}
				>{#snippet children(items)}<CareAssignmentList {items} term={data.term} /><Button
						variant="outline"
						href={`/staff/student-care/students/${data.id}/assessments`}
						>ดูประวัติแบบประเมินทั้งหมด</Button
					>{/snippet}</CareRegion
			></CareSection
		>{#if $can.hasAny(...careRoutineReadPermissions)}<CareSection title="เยี่ยมบ้าน" icon={House}
				><CareRegion
					initial={data.visits}
					permissions={careRoutineReadPermissions}
					retry={(signal) => careVisits(data.query, { signal })}
					>{#snippet children(items)}{#each items as item (item.id)}<div
								class="flex flex-wrap items-center justify-between gap-3 border-b py-3"
							>
								<span
									>{item.studentName} · {careDate(item.plannedDate)} · {visitLabels[
										item.status
									]}</span
								><Button href={careLink(`/visits/${item.id}`, data.term)} variant="outline"
									>ดูบันทึกเยี่ยมบ้าน</Button
								>
							</div>{/each}{#if !items.length}<p class="text-sm text-muted-foreground">
								ยังไม่มีรายการเยี่ยมบ้านในขอบเขตของคุณ
							</p>{/if}<Button
							href={`/staff/student-care/visits?studentId=${data.id}`}
							variant="ghost">ดูประวัติเยี่ยมบ้านทั้งหมด</Button
						>{/snippet}</CareRegion
				></CareSection
			>{/if}{#if $can.hasAny(...careSensitiveReadPermissions)}<CareSection
				title="การดูแลรายกรณี"
				icon={HeartHandshake}
				><CareRegion
					initial={data.cases}
					permissions={careSensitiveReadPermissions}
					retry={(signal) => careCases(data.query, { signal })}
					>{#snippet children(items)}{#each items as item (item.id)}<div
								class="flex flex-wrap items-center justify-between gap-3 border-b py-3"
							>
								<span>{item.title} · {caseLabels[item.status]}</span><Button
									href={careLink(`/cases/${item.id}`, data.term)}
									variant="outline">ดูการดูแล</Button
								>
							</div>{/each}{#if !items.length}<p class="text-sm text-muted-foreground">
								ยังไม่มีการดูแลในขอบเขตของคุณ
							</p>{/if}<Button
							href={`/staff/student-care/cases?studentId=${data.id}`}
							variant="ghost">ดูประวัติการดูแลทั้งหมด</Button
						>{/snippet}</CareRegion
				></CareSection
			>{/if}{/key}</PageShell
>
