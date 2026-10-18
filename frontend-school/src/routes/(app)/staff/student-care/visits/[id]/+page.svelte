<script lang="ts">
	import { House } from '@lucide/svelte';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';
	import { Button } from '#lib/components/ui/button/index.js';

	import { can } from '#lib/stores/permissions.js';

	import {
		mayManageCareVisit,
		careIdentity,
		careRoutineReadPermissions,
		careSensitiveManagePermissions
	} from '#lib/features/student-care/access.js';
	import { careLink, careDate } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';

	import CareStatus from '#lib/components/student-care/CareStatus.svelte';
	import CareSection from '#lib/components/student-care/CareSection.svelte';
	import { careVisit } from '#lib/api/student-care.js';
	let { data }: PageProps = $props();
</script>

<PageShell
	backLabel="กลับรายการเยี่ยมบ้าน"
	icon={House}
	title="รายละเอียดเยี่ยมบ้าน"
	backHref={careLink('/visits')}
	>{#key $careIdentity + data.id}<CareRegion
			initial={data.initial}
			permissions={careRoutineReadPermissions}
			retry={(signal) => careVisit(data.id, { signal })}
			variant="detail"
			>{#snippet children(record)}<CareSection title={record.visit.studentName} icon={House}
					><CareStatus
						status={{ kind: 'visit', value: record.visit.status }}
					/>{#if mayManageCareVisit(record.visit) && record.visit.status !== 'cancelled'}<Button
							href={careLink(`/visits/${data.id}/edit`, record.visit.academicTermId)}
							>แก้ไขบันทึก</Button
						>{/if}
					<dl class="grid gap-3 text-sm sm:grid-cols-2">
						<div>
							<dt class="text-muted-foreground">นัดหมาย</dt>
							<dd class="break-words">{careDate(record.visit.plannedDate)}</dd>
						</div>
						<div>
							<dt class="text-muted-foreground">วันที่เยี่ยมจริง</dt>
							<dd class="break-words">{careDate(record.visitedDate)}</dd>
						</div>
						<div>
							<dt class="text-muted-foreground">ผู้ให้ข้อมูล</dt>
							<dd class="break-words">{record.contactPerson || 'ยังไม่ระบุ'}</dd>
						</div>
						<div>
							<dt class="text-muted-foreground">ติดตามครั้งต่อไป</dt>
							<dd class="break-words">{careDate(record.nextFollowUp)}</dd>
						</div>
					</dl></CareSection
				><CareSection title="บันทึกการเยี่ยมบ้าน"
					>{#each [{ label: 'ข้อเท็จจริงที่สังเกต', value: record.factualObservations }, { label: 'จุดแข็งและสิ่งสนับสนุน', value: record.strengths }, { label: 'ความต้องการการช่วยเหลือ', value: record.needs }, { label: 'แนวทางที่ตกลงร่วมกัน', value: record.agreedActions }] as field (field.label)}<section
							class="space-y-1"
						>
							<h3 class="text-sm font-medium">{field.label}</h3>
							<p class="whitespace-pre-wrap break-words text-sm text-muted-foreground">
								{field.value || 'ยังไม่บันทึก'}
							</p>
						</section>{/each}{#if $can.hasAny(...careSensitiveManagePermissions)}<Button
							href={`/staff/student-care/cases/new?academicTermId=${record.visit.academicTermId}&sourceVisitId=${record.visit.id}&studentId=${record.visit.studentId}`}
							>เปิดการดูแลจากเยี่ยมบ้าน</Button
						>{/if}</CareSection
				>{/snippet}</CareRegion
		>{/key}</PageShell
>
