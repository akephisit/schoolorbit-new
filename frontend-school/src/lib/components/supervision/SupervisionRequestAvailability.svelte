<script lang="ts">
	import type { SupervisionEvaluatorAvailabilityResponse } from '#lib/api/supervision.js';
	import { supervisionStatusLabel } from '#lib/supervision/supervision-presentation.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { PageState } from '#lib/components/app-state/index.js';
	import { Skeleton } from '#lib/components/ui/skeleton/index.js';
	import { cn } from '#lib/utils.js';
	let {
		result,
		loading,
		error,
		showError,
		disabled,
		onretry
	}: {
		result?: SupervisionEvaluatorAvailabilityResponse;
		loading: boolean;
		error: string;
		showError: boolean;
		disabled: boolean;
		onretry: () => void;
	} = $props();
	const availableCount = $derived(result?.items.filter((item) => item.available).length ?? 0);
	const overlapCount = $derived(
		(result?.overlap.pendingCount ?? 0) + (result?.overlap.confirmedCount ?? 0)
	);
	function timeLabel(value: string) {
		return new Intl.DateTimeFormat('th-TH', {
			timeZone: 'Asia/Bangkok',
			dateStyle: 'medium',
			timeStyle: 'short'
		}).format(new Date(value));
	}
</script>

<div class="min-w-0 space-y-2" data-testid="supervision-request-availability" aria-busy={loading}>
	{#if !result && loading}
		<div
			class="space-y-2 rounded-md border px-3 py-2.5"
			role="status"
			aria-label="กำลังตรวจช่วงคาบและผู้ประเมิน"
		>
			<Skeleton class="h-4 w-2/3" />
			<Skeleton class="h-3 w-1/2" />
		</div>
	{/if}
	{#if error && showError}<PageState
			variant="error"
			title="ตรวจช่วงคาบและผู้ประเมินไม่สำเร็จ"
			description={error}
		>
			{#snippet action()}
				<Button size="sm" variant="outline" disabled={loading || disabled} onclick={onretry}>
					ลองใหม่
				</Button>
			{/snippet}
		</PageState>{/if}
	{#if result}
		<div class="space-y-2 rounded-md border bg-muted/20 px-3 py-2.5 text-sm">
			<div class="flex flex-wrap items-center gap-x-2 gap-y-1">
				{#if error || loading}<p class="text-xs font-medium text-muted-foreground">
						ผลตรวจครั้งล่าสุด
					</p>{/if}
				<Badge
					variant="outline"
					class={cn(
						'max-w-full whitespace-normal',
						result.overlap.blocked
							? 'border-destructive/30 bg-destructive/5 text-destructive'
							: 'border-success/30 bg-success/10 text-success'
					)}
					>{result.overlap.blocked
						? 'มีนัดทับกัน อนุมัติไม่ได้'
						: 'ไม่พบรายการที่กีดกันการอนุมัติ'}</Badge
				>
			</div>
			{#if overlapCount}
				<p class="text-xs text-muted-foreground">
					รออนุมัติ {result.overlap.pendingCount} รายการ · ยืนยันแล้ว {result.overlap
						.confirmedCount} รายการ
				</p>
			{/if}
			{#if !result.withinCycleDates}<p
					role="alert"
					class="rounded-md border border-destructive/20 bg-destructive/5 p-2.5 text-destructive"
				>
					คาบนี้อยู่นอกช่วงนิเทศหรือรอบยังไม่กำหนดวันที่ กำหนดช่วงวันที่ใหม่หรือส่งกลับคำขอ
				</p>{/if}
			{#if result.overlappingBookings.length}<ul
					class="space-y-2 border-t pt-2"
					aria-label="รายการนิเทศที่ทับช่วงคาบและมีสิทธิ์ดู"
				>
					{#each result.overlappingBookings as booking, index (index)}<li
							class="flex flex-wrap items-start justify-between gap-2"
						>
							<div class="min-w-0">
								<p class="font-medium break-words">
									{booking.observedDisplayName ?? 'รายการนิเทศ'}
								</p>
								<p class="text-xs text-muted-foreground">
									{timeLabel(booking.observedAt)}{#if booking.endsAt}
										– {new Date(booking.endsAt).toLocaleTimeString('th-TH', {
											timeZone: 'Asia/Bangkok',
											hour: '2-digit',
											minute: '2-digit'
										})}{/if}
								</p>
							</div>
							<Badge variant="secondary">{supervisionStatusLabel(booking.status)}</Badge>
						</li>{/each}
				</ul>{/if}
			{#if overlapCount > result.overlappingBookings.length}<p
					class="text-xs text-muted-foreground"
				>
					รายละเอียดแสดงเฉพาะรายการที่มีสิทธิ์ดู
				</p>{/if}
			<p class="text-xs text-muted-foreground">
				ผู้ประเมินว่างตลอดช่วงคาบ {availableCount} คน · ไม่ว่าง {result.items.length -
					availableCount} คน ตรวจจากนัดนิเทศที่ยืนยันแล้วตลอดช่วงคาบที่เลือก
			</p>
		</div>
	{/if}
</div>
