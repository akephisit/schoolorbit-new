<script lang="ts">
	import { can } from '#lib/stores/permissions.js';
	import {
		careRoutineReadPermissions,
		careSensitiveReadPermissions
	} from '#lib/features/student-care/access.js';
	import { careToday } from '#lib/features/student-care/presentation.js';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';

	import { careIdentity, careReadPermissions } from '#lib/features/student-care/access.js';
	import { careLink } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';
	import CareNavigation from '#lib/components/student-care/CareNavigation.svelte';

	import CareSection from '#lib/components/student-care/CareSection.svelte';
	import {
		HeartHandshake,
		ClipboardList,
		FileCheck,
		House,
		CalendarClock,
		BellRing,
		Plus
	} from '@lucide/svelte';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	import { careOverview, careAssignments } from '#lib/api/student-care.js';
	import CareAssignmentList from '#lib/components/student-care/CareAssignmentList.svelte';
	let { data }: PageProps = $props();
</script>

<PageShell
	title="ดูแลช่วยเหลือนักเรียน"
	description="ทุกภาคเรียนในขอบเขตที่คุณรับผิดชอบ"
	icon={HeartHandshake}
	>{#snippet actions()}{#if $can.has(PERMISSIONS.STUDENT_CARE_MANAGE_SCHOOL)}<Button
				href={careLink('/rounds/new', data.term)}><Plus aria-hidden="true" />สร้างรอบประเมิน</Button
			>{/if}{/snippet}<CareNavigation term={data.term} />{#key $careIdentity}<CareRegion
			initial={data.overview}
			permissions={careReadPermissions}
			retry={(signal) => careOverview({ signal })}
			variant="detail"
			label="ภาพรวม"
			>{#snippet children(overview)}<section
					aria-label="สรุปงานดูแล"
					class="grid gap-3 sm:grid-cols-2 xl:grid-cols-3"
				>
					{#each [{ label: 'รอส่งแบบประเมิน', value: overview.assignedCount, icon: ClipboardList }, { label: 'ส่งแบบประเมินแล้ว', value: overview.submittedCount, icon: FileCheck }, ...($can.hasAny(...careRoutineReadPermissions) ? [{ label: 'แผนเยี่ยมบ้านคงค้าง', value: overview.pendingVisitCount, icon: House }] : []), ...($can.hasAny(...careSensitiveReadPermissions) ? [{ label: 'การดูแลที่ยังเปิด', value: overview.openCaseCount, icon: HeartHandshake }, { label: 'ถึงกำหนดติดตาม', value: overview.followUpDueCount, icon: CalendarClock }, { label: 'รอผู้รับผิดชอบรับทราบ', value: overview.urgentUnacknowledgedCount, icon: BellRing }] : [])] as metric (metric.label)}<div
							class="rounded-xl border border-primary/15 bg-gradient-to-br from-primary/10 via-card to-card p-4"
						>
							<div class="flex items-center justify-between gap-3">
								<p class="text-sm text-muted-foreground">{metric.label}</p>
								<metric.icon class="size-5 shrink-0 text-primary" aria-hidden="true" />
							</div>
							<p class="mt-2 text-2xl font-semibold">{metric.value}</p>
						</div>{/each}
				</section>
				{#if $can.hasAny(...careSensitiveReadPermissions)}<Button
						href={`/staff/student-care/cases?followUpBefore=${careToday()}`}
						variant="outline">งานติดตามถึงกำหนด</Button
					>{/if}{/snippet}</CareRegion
		><CareSection
			title="แบบประเมินในขอบเขตของคุณ"
			icon={ClipboardList}
			description="10 รายการแรกในขอบเขตของคุณ ดูการมอบหมายแต่ละรอบได้ที่รอบประเมิน"
			><CareRegion
				initial={data.assignments}
				permissions={careReadPermissions}
				retry={(signal) => careAssignments({ limit: 10 }, { signal })}
				label="แบบประเมิน"
				>{#snippet children(items)}<CareAssignmentList
						{items}
						term={data.term}
					/>{/snippet}</CareRegion
			></CareSection
		>{/key}</PageShell
>
