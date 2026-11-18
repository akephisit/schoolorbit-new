<script lang="ts">
	import type { SupervisionEvaluatorAvailabilityResponse } from '#lib/api/supervision.js';
	import { supervisionStatusLabel } from '#lib/supervision/supervision-presentation.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { PageSkeleton, PageState } from '#lib/components/app-state/index.js';
	let {
		result,
		loading,
		error,
		showError,
		disabled,
		oninspect
	}: {
		result?: SupervisionEvaluatorAvailabilityResponse;
		loading: boolean;
		error: string;
		showError: boolean;
		disabled: boolean;
		oninspect: () => void;
	} = $props();
	function timeLabel(value: string) {
		return new Intl.DateTimeFormat('th-TH', {
			timeZone: 'Asia/Bangkok',
			dateStyle: 'medium',
			timeStyle: 'short'
		}).format(new Date(value));
	}
</script>

<div class="mt-4 space-y-3" data-testid="supervision-request-availability" aria-busy={loading}>
	{#if !result && loading}<PageSkeleton rows={2} columns={2} />{/if}
	{#if error && showError}<PageState
			variant="error"
			title="ตรวจช่วงคาบและผู้ประเมินไม่สำเร็จ"
			description={error}
		/>{/if}
	{#if result}
		<div class="space-y-2 rounded-lg border bg-muted/20 p-3 text-sm">
			<div class="flex flex-wrap items-center gap-2">
				<p class="font-medium">{error ? 'ผลการตรวจครั้งล่าสุด' : 'รายการที่ทับช่วงคาบนี้'}</p>
				<Badge variant={result.overlap.blocked ? 'destructive' : 'outline'}
					>{result.overlap.blocked
						? 'มีนัดทับกัน อนุมัติไม่ได้'
						: 'ไม่พบรายการที่กีดกันการอนุมัติ'}</Badge
				>
			</div>
			<p>
				รออนุมัติ {result.overlap.pendingCount} รายการ · ยืนยันแล้ว {result.overlap.confirmedCount} รายการ
			</p>
			{#if !result.withinCycleDates}<p role="alert" class="text-destructive">
					คาบเดิมอยู่นอกช่วงวันที่นิเทศหรือรอบยังไม่กำหนดวันที่
					ปรับช่วงวันที่ของรอบหรือส่งกลับคำขอก่อนอนุมัติ
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
			{#if result.overlap.pendingCount + result.overlap.confirmedCount > result.overlappingBookings.length}<p
					class="text-xs text-muted-foreground"
				>
					แสดงเฉพาะรายการในขอบเขตที่มีสิทธิ์ดู จำนวนรวมอาจมากกว่ารายการที่แสดง
				</p>{/if}
			<p class="text-xs text-muted-foreground">
				ผู้ประเมินว่าง {result.items.filter((item) => item.available).length} คน · ไม่ว่าง {result.items.filter(
					(item) => !item.available
				).length} คน ตรวจจากงานนิเทศและตารางสอนที่ระบบมีข้อมูลตลอดช่วงคาบที่เลือก
			</p>
		</div>
	{/if}
	{#if result || !loading}<Button
			size="sm"
			variant="outline"
			disabled={loading || disabled}
			onclick={oninspect}>{result || error ? 'ตรวจอีกครั้ง' : 'ตรวจช่วงคาบและผู้ประเมิน'}</Button
		>{/if}
</div>
