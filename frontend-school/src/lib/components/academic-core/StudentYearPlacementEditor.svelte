<script lang="ts">
	import type { Homeroom, HomeroomPlacement, StudentAcademicYear } from '$lib/api/academic-core';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { DatePicker } from '$lib/components/ui/date-picker';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Select from '$lib/components/ui/select';
	import { ArrowRightLeft, CalendarPlus, History } from 'lucide-svelte';
	import StudentYearTransferDialog from './StudentYearTransferDialog.svelte';

	let {
		studentYear,
		placements,
		homerooms,
		canManage = false,
		onCreatePlacement,
		onTransfer
	}: {
		studentYear: StudentAcademicYear;
		placements: HomeroomPlacement[];
		homerooms: Homeroom[];
		canManage?: boolean;
		onCreatePlacement: (draft: {
			homeroomId: string;
			startDate: string;
			enrollmentType: string;
			classNumber: number | null;
		}) => Promise<void>;
		onTransfer: (
			placement: HomeroomPlacement,
			draft: {
				targetHomeroomId: string;
				transferDate: string;
				enrollmentType: string;
				classNumber: number | null;
				reason: string;
			}
		) => Promise<import('$lib/api/academic-core').HomeroomPlacementTransfer>;
	} = $props();

	let draft = $state({
		homeroomId: '',
		startDate: '',
		enrollmentType: 'promotion',
		classNumber: null as number | null
	});
	let busy = $state(false);
	let errorMessage = $state('');
	let transferPlacement = $state<HomeroomPlacement | null>(null);
	const currentPlacement = $derived(
		placements.find((placement) => placement.status === 'current') ?? null
	);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (!draft.homeroomId || !draft.startDate) {
			errorMessage = 'กรุณาเลือกห้องและวันที่เริ่ม';
			return;
		}
		busy = true;
		errorMessage = '';
		try {
			await onCreatePlacement(draft);
			draft = { ...draft, homeroomId: '', startDate: '', classNumber: null };
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'จัดห้องไม่สำเร็จ';
		} finally {
			busy = false;
		}
	}
</script>

<section class="rounded-xl border bg-card">
	<header class="flex flex-wrap items-center justify-between gap-3 border-b px-5 py-4">
		<div>
			<div class="flex items-center gap-2">
				<h2 class="font-semibold">นักเรียน {studentYear.studentId}</h2>
				<Badge variant="outline">{studentYear.status}</Badge>
			</div>
			<p class="text-xs text-muted-foreground">
				ระดับชั้น {studentYear.gradeLevelId} · แผน {studentYear.studyProgramId}
			</p>
		</div>
		{#if canManage && currentPlacement}<Button
				size="sm"
				variant="outline"
				onclick={() => (transferPlacement = currentPlacement)}
				><ArrowRightLeft class="size-4" /> ย้ายห้อง</Button
			>{/if}
	</header>
	<div class="grid gap-5 p-5 lg:grid-cols-[minmax(0,1fr)_300px]">
		<div>
			<div class="mb-3 flex items-center gap-2 text-sm font-medium">
				<History class="size-4 text-primary" /> ประวัติการจัดห้อง
			</div>
			<ol class="relative ml-2 space-y-4 border-l pl-5">
				{#each placements as placement (placement.id)}
					<li class="relative">
						<span
							class="absolute -left-[26px] top-1 size-3 rounded-full border-2 border-background bg-primary"
						></span>
						<div class="flex flex-wrap items-center gap-2">
							<p class="font-medium">
								{homerooms.find((room) => room.id === placement.homeroomId)?.name ??
									placement.homeroomId}
							</p>
							<Badge variant={placement.status === 'current' ? 'default' : 'secondary'}
								>{placement.status}</Badge
							>
						</div>
						<p class="text-xs text-muted-foreground">
							{placement.startDate} – {placement.endDate ?? 'ปัจจุบัน'} · {placement.enrollmentType}
						</p>
					</li>
				{:else}<li class="text-sm text-muted-foreground">ยังไม่มีประวัติการจัดห้อง</li>{/each}
			</ol>
		</div>
		{#if canManage && !currentPlacement}
			<form class="space-y-3 rounded-lg border bg-muted/20 p-4" onsubmit={submit}>
				<div class="flex items-center gap-2">
					<CalendarPlus class="size-4 text-primary" />
					<h3 class="font-medium">สร้างรายการจัดห้อง</h3>
				</div>
				<div class="space-y-1.5">
					<Label for={`placement-room-${studentYear.id}`}>ห้องประจำชั้น</Label>
					<Select.Root type="single" bind:value={draft.homeroomId}>
						<Select.Trigger id={`placement-room-${studentYear.id}`} class="w-full">
							{homerooms.find((room) => room.id === draft.homeroomId)?.name ?? 'เลือกห้อง'}
						</Select.Trigger>
						<Select.Content>
							{#each homerooms as room (room.id)}
								<Select.Item value={room.id}>{room.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>
				<div class="space-y-1.5">
					<Label for={`placement-date-${studentYear.id}`}>วันที่เริ่ม</Label>
					<DatePicker
						id={`placement-date-${studentYear.id}`}
						bind:value={draft.startDate}
						ariaLabel="เลือกวันที่เริ่มจัดห้อง"
						required
					/>
				</div>
				<div class="space-y-1.5">
					<Label for={`placement-number-${studentYear.id}`}>เลขที่</Label><Input
						id={`placement-number-${studentYear.id}`}
						type="number"
						min="1"
						bind:value={draft.classNumber}
					/>
				</div>
				<Button type="submit" class="w-full" disabled={busy}>บันทึกการจัดห้อง</Button>
			</form>
		{/if}
	</div>
	{#if errorMessage}<p role="alert" class="border-t px-5 py-3 text-sm text-destructive">
			{errorMessage}
		</p>{/if}
</section>

<StudentYearTransferDialog
	open={transferPlacement !== null}
	placement={transferPlacement}
	{homerooms}
	onClose={() => (transferPlacement = null)}
	onTransfer={(draft) => {
		if (!transferPlacement) throw new Error('ไม่พบรายการจัดห้องปัจจุบัน');
		return onTransfer(transferPlacement, draft);
	}}
/>
