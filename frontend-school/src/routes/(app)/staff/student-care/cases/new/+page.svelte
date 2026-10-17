<script lang="ts">
	import { careCaseSetup } from '#lib/features/student-care/case-setup.js';
	import type { PageProps } from './$types';
	import { PageShell } from '#lib/components/app-layout/index.js';

	import { PageState } from '#lib/components/app-state/index.js';

	import {
		careIdentity,
		careSensitiveManagePermissions
	} from '#lib/features/student-care/access.js';
	import { careLink } from '#lib/features/student-care/presentation.js';
	import CareRegion from '#lib/components/student-care/CareRegion.svelte';

	import CareCaseForm from '#lib/components/student-care/CareCaseForm.svelte';
	let { data }: PageProps = $props();
	async function retry(signal: AbortSignal) {
		return careCaseSetup(data, { signal });
	}
</script>

<PageShell title="เปิดการดูแลรายกรณี" backHref={careLink('/cases', data.term)}
	>{#key $careIdentity + data.sourceAssignmentId + data.sourceVisitId + data.term}<CareRegion
			initial={data.initial}
			permissions={careSensitiveManagePermissions}
			{retry}
			variant="form"
			>{#snippet children(options)}{#if options.sourceRequired}<PageState
						title="เริ่มจากข้อมูลที่คุณรับผิดชอบ"
						description="เปิดการดูแลจากหน้าแบบประเมินหรือบันทึกเยี่ยมบ้านที่ได้รับมอบหมาย"
					/>{:else if options.term}<p class="text-sm text-muted-foreground">
						{options.studentName}
					</p>
					<CareCaseForm
						students={options.students}
						staff={options.staff}
						term={options.term}
						date={data.date}
						studentId={options.studentId}
						sourceAssignmentId={data.sourceAssignmentId}
						sourceVisitId={data.sourceVisitId}
					/>{:else}<PageState
						title="เลือกภาคเรียนก่อนเปิดการดูแล"
						description="เริ่มจากรายการรอบประเมินหรือเยี่ยมบ้าน เพื่อเลือกนักเรียนและเชื่อมโยงข้อมูลต้นทาง"
						actionLabel="เปิดรอบประเมิน"
						href={careLink('/rounds')}
					/>{/if}{/snippet}</CareRegion
		>{/key}</PageShell
>
