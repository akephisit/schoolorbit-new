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
	const activePlacement = $derived(
		placements.find((placement) => ['current', 'planned'].includes(placement.status)) ?? null
	);
	const currentPlacement = $derived(
		placements.find((placement) => placement.status === 'current') ?? null
	);

	function roomName(homeroomId: string): string | null {
		return homerooms.find((room) => room.id === homeroomId)?.name ?? null;
	}

	function placementStatus(status: HomeroomPlacement['status']): string {
		return {
			planned: 'เตรียมการ',
			current: 'ห้องปัจจุบัน',
			ended: 'สิ้นสุดแล้ว',
			cancelled: 'ยกเลิก'
		}[status];
	}

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

<div class="space-y-5">
	<div class="grid gap-3 rounded-xl border bg-muted/20 p-4 sm:grid-cols-3">
		<div>
			<span class="text-xs text-muted-foreground">ระดับชั้น</span><strong class="block text-sm"
				>{studentYear.gradeLevelName}</strong
			>
		</div>
		<div>
			<span class="text-xs text-muted-foreground">แผนการเรียน</span><strong class="block text-sm"
				>{studentYear.studyProgramName}</strong
			>
		</div>
		<div>
			<span class="text-xs text-muted-foreground">ห้องปัจจุบัน</span><strong
				class={[
					'block text-sm',
					activePlacement && !roomName(activePlacement.homeroomId) ? 'text-destructive' : ''
				]}
				>{activePlacement
					? (roomName(activePlacement.homeroomId) ?? 'ไม่พบห้องประจำชั้นที่อ้างอิง')
					: 'ยังไม่ได้จัดห้อง'}</strong
			>
		</div>
	</div>

	<div class="flex flex-wrap items-center justify-between gap-3">
		<div class="flex items-center gap-2 text-sm font-medium">
			<History class="size-4 text-primary" /> ประวัติการจัดห้อง
		</div>
		{#if canManage && currentPlacement}<Button
				type="button"
				size="sm"
				variant="outline"
				onclick={() => (transferPlacement = currentPlacement)}
				><ArrowRightLeft class="size-4" /> ย้ายห้อง</Button
			>{/if}
	</div>

	<ol class="relative ms-2 space-y-4 border-s ps-5">
		{#each placements as placement (placement.id)}
			<li class="relative">
				<span
					class="absolute -start-[26px] top-1 size-3 rounded-full border-2 border-background bg-primary"
				></span>
				<div class="flex flex-wrap items-center gap-2">
					<p class={['font-medium', roomName(placement.homeroomId) ? '' : 'text-destructive']}>
						{roomName(placement.homeroomId) ?? 'ไม่พบห้องประจำชั้นที่อ้างอิง'}
					</p>
					<Badge variant={placement.status === 'current' ? 'default' : 'secondary'}
						>{placementStatus(placement.status)}</Badge
					>
					{#if placement.classNumber}<Badge variant="outline">เลขที่ {placement.classNumber}</Badge
						>{/if}
				</div>
				<p class="text-xs text-muted-foreground">
					{placement.startDate} – {placement.endDate ?? 'ปัจจุบัน'}
				</p>
			</li>
		{:else}
			<li class="text-sm text-muted-foreground">ยังไม่มีประวัติการจัดห้อง</li>
		{/each}
	</ol>

	{#if canManage && !activePlacement}
		<form class="space-y-4 rounded-xl border bg-card p-4" onsubmit={submit}>
			<div class="flex items-center gap-2">
				<CalendarPlus class="size-4 text-primary" />
				<h3 class="font-medium">จัดห้องครั้งแรก</h3>
			</div>
			<div class="grid gap-4 sm:grid-cols-3">
				<div class="space-y-1.5 sm:col-span-2">
					<Label for={`placement-room-${studentYear.id}`}>ห้องประจำชั้น</Label>
					<Select.Root type="single" bind:value={draft.homeroomId}>
						<Select.Trigger id={`placement-room-${studentYear.id}`} class="w-full"
							>{homerooms.find((room) => room.id === draft.homeroomId)?.name ??
								'เลือกห้อง'}</Select.Trigger
						>
						<Select.Content
							>{#each homerooms.filter((room) => room.gradeLevelId === studentYear.gradeLevelId && room.studyProgramId === studentYear.studyProgramId) as room (room.id)}<Select.Item
									value={room.id}>{room.name}</Select.Item
								>{/each}</Select.Content
						>
					</Select.Root>
				</div>
				<div class="space-y-1.5">
					<Label for={`placement-number-${studentYear.id}`}>เลขที่</Label><Input
						id={`placement-number-${studentYear.id}`}
						type="number"
						min="1"
						bind:value={draft.classNumber}
					/>
				</div>
				<div class="space-y-1.5 sm:col-span-2">
					<Label for={`placement-date-${studentYear.id}`}>วันที่เริ่ม</Label><DatePicker
						id={`placement-date-${studentYear.id}`}
						bind:value={draft.startDate}
						ariaLabel="เลือกวันที่เริ่มจัดห้อง"
						required
					/>
				</div>
			</div>
			{#if errorMessage}<p role="alert" class="text-sm text-destructive">{errorMessage}</p>{/if}
			<Button type="submit" disabled={busy}>บันทึกการจัดห้อง</Button>
		</form>
	{/if}
</div>

<StudentYearTransferDialog
	open={transferPlacement !== null}
	placement={transferPlacement}
	{homerooms}
	onClose={() => (transferPlacement = null)}
	onTransfer={(transferDraft) => {
		if (!transferPlacement) throw new Error('ไม่พบรายการจัดห้องปัจจุบัน');
		return onTransfer(transferPlacement, transferDraft);
	}}
/>
