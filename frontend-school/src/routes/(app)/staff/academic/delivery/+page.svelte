<script lang="ts">
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { getAcademicContextStore } from '$lib/academic-context/store';
	import {
		getLearningDeliveryOverview,
		type LearningDeliveryOverview,
		type LearningOfferingOverviewItem
	} from '$lib/api/learning-delivery';
	import { LatestRequest, isAbortError } from '$lib/async/latest-request';
	import { PageShell } from '$lib/components/app-layout';
	import { PageSkeleton, PageState } from '$lib/components/app-state';
	import {
		AcademicPrerequisiteNotice,
		type AcademicPrerequisite
	} from '$lib/components/academic-workflow';
	import OfferingCreateDialog from '$lib/components/learning-delivery/OfferingCreateDialog.svelte';
	import OfferingOverviewTable from '$lib/components/learning-delivery/OfferingOverviewTable.svelte';
	import { PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';

	const academicContext = getAcademicContextStore();
	const academicTermId = $derived($academicContext.selected.academicTermId);
	const request = new LatestRequest();
	let overview = $state.raw<LearningDeliveryOverview | null>(null);
	let loading = $state(false);
	let errorMessage = $state('');
	let initialKind = $derived<'all' | 'activity'>(
		page.url.searchParams.get('kind') === 'activity' ? 'activity' : 'all'
	);
	let canManage = $derived(
		$can.hasAny(
			PERMISSIONS.LEARNING_OFFERING_MANAGE_SCHOOL,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.LEARNING_OFFERING_MANAGE_ASSIGNED
		)
	);
	let items = $derived(overview?.offerings ?? []);

	const missingTermPrerequisite: AcademicPrerequisite = {
		key: 'academic-term',
		status: 'missing',
		title: 'เลือกปีการศึกษาและภาคเรียนก่อน',
		description: 'รายการเปิดสอน กลุ่มเรียน ครู และรายชื่อนักเรียนแยกกันในแต่ละภาคเรียน',
		actionLabel: 'ไปตั้งค่าปีและภาคเรียน',
		href: '/staff/academic/core'
	};
	const noOfferingPrerequisite: AcademicPrerequisite = {
		key: 'learning-offerings',
		status: 'warning',
		title: 'ภาคเรียนนี้ยังไม่มีรายการเปิดสอน',
		description: 'นำรายวิชาและกิจกรรมจากหลักสูตรมาใช้ หรือเพิ่มรายการเฉพาะภาคเรียนนี้ได้',
		actionLabel: 'ตรวจหลักสูตรและแผนการเรียน',
		href: '/staff/academic/curricula'
	};

	async function loadOverview(termId: string) {
		const { revision, signal } = request.begin();
		loading = true;
		errorMessage = '';
		try {
			const result = await getLearningDeliveryOverview(termId, { signal });
			if (!request.isCurrent(revision)) return;
			overview = result;
		} catch (error) {
			if (isAbortError(error)) return;
			if (request.isCurrent(revision))
				errorMessage = error instanceof Error ? error.message : 'โหลดภาพรวมรายการเปิดสอนไม่สำเร็จ';
		} finally {
			if (request.isCurrent(revision)) loading = false;
		}
	}

	function addCreated(item: LearningOfferingOverviewItem) {
		if (!overview) {
			overview = { academicTermId: item.offering.academicTermId, offerings: [item] };
			return;
		}
		overview = {
			...overview,
			offerings: [...overview.offerings, item].sort((left, right) =>
				left.offering.codeSnapshot.localeCompare(right.offering.codeSnapshot, 'th-TH', {
					numeric: true
				})
			)
		};
	}

	async function reloadAfterApply() {
		if (academicTermId) await loadOverview(academicTermId);
	}

	onMount(() => {
		let loadedTermId: string | null = null;
		const unsubscribe = academicContext.subscribe((state) => {
			const termId = state.selected.academicTermId;
			if (termId && termId !== loadedTermId) {
				loadedTermId = termId;
				void loadOverview(termId);
			} else if (!termId) {
				loadedTermId = null;
				overview = null;
				loading = false;
				errorMessage = '';
				request.abort();
			}
		});
		return () => {
			unsubscribe();
			request.abort();
		};
	});
</script>

<PageShell
	title="เปิดการเรียนการสอน"
	description="จัดรายการเปิดสอนของภาคเรียน แล้วลงรายละเอียดกลุ่มเรียน ครู ห้อง และรายชื่อนักเรียน"
>
	{#snippet actions()}
		{#if canManage && academicTermId}
			<OfferingCreateDialog {academicTermId} onCreated={addCreated} onApplied={reloadAfterApply} />
		{/if}
	{/snippet}

	{#if !academicTermId}
		<AcademicPrerequisiteNotice prerequisite={missingTermPrerequisite} />
	{:else if loading && !overview}
		<PageSkeleton variant="table" rows={7} />
	{:else if errorMessage && !overview}
		<PageState
			variant="error"
			title="โหลดรายการเปิดสอนไม่สำเร็จ"
			description={errorMessage}
			actionLabel="ลองอีกครั้ง"
			onaction={() => loadOverview(academicTermId)}
		/>
	{:else}
		<div class="space-y-4">
			{#if items.length === 0}
				<AcademicPrerequisiteNotice prerequisite={noOfferingPrerequisite} />
			{:else}
				<section class="overflow-hidden rounded-2xl border bg-card shadow-sm">
					<div class="flex flex-wrap items-start justify-between gap-4 border-b bg-muted/25 p-4">
						<div>
							<h2 class="font-semibold">รายการเปิดสอนของภาคเรียน</h2>
							<p class="mt-1 text-sm text-muted-foreground">
								เลือกแต่ละรายการเพื่อจัดกลุ่มเรียน มอบหมายครู เลือกห้อง และเผยแพร่รายชื่อ
							</p>
						</div>
						<p class="rounded-full bg-primary/10 px-3 py-1 text-sm font-medium text-primary">
							{items.length} รายการ
						</p>
					</div>
					<OfferingOverviewTable {items} {initialKind} />
				</section>
			{/if}
			<p class="text-xs text-muted-foreground">
				รายการเปิดสอนเป็นข้อมูลของภาคเรียนที่เลือกบนแถบด้านบน
				การเปลี่ยนภาคเรียนจะโหลดพื้นที่ทำงานของภาคเรียนนั้นโดยตรง
			</p>
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
		</div>
	{/if}
</PageShell>
