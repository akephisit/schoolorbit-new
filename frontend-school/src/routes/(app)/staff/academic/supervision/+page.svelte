<script lang="ts">
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import {
		AcademicPrerequisiteNotice,
		type AcademicPrerequisite
	} from '$lib/components/academic-workflow';
	import SupervisionWorkspace from '$lib/components/supervision/SupervisionWorkspace.svelte';

	const academicContext = getAcademicContextStore();
	const academicTermId = $derived($academicContext.selected.academicTermId);
	const bookingTermPrerequisite: AcademicPrerequisite = {
		key: 'supervision-booking-term',
		status: 'missing',
		title: 'เลือกภาคเรียนก่อนจองคาบนิเทศ',
		description:
			'รอบนิเทศและแบบประเมินยังดูได้ตามสิทธิ์ แต่การจองคาบต้องทราบภาคเรียนเพื่อค้นหากลุ่ม ครู และตารางสอน',
		actionLabel: 'ตรวจพื้นที่จัดตารางสอน',
		href: '/staff/academic/timetable'
	};
</script>

{#if !academicTermId}
	<AcademicPrerequisiteNotice
		prerequisite={bookingTermPrerequisite}
		class="mx-4 mt-4 lg:mx-6 lg:mt-6"
	/>
{/if}

<SupervisionWorkspace section="mine" />
