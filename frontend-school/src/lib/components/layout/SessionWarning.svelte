<script lang="ts">
	import type { SessionWarning } from '#lib/auth/session-activity.js';
	import { Alert, AlertDescription, AlertTitle } from '#lib/components/ui/alert/index.js';
	import LoadingButton from '#lib/components/app-state/LoadingButton.svelte';
	let {
		warning,
		oncontinue
	}: { warning: SessionWarning | null; oncontinue: (event: MouseEvent) => void } = $props();
</script>

{#if warning}
	<div class="px-4 pt-3 sm:px-6" data-testid="session-warning">
		<Alert class="border-warning/40 bg-warning/10">
			<AlertTitle>ระยะเวลาการใช้งานใกล้สิ้นสุด</AlertTitle>
			<AlertDescription class="flex flex-wrap items-center justify-between gap-3">
				<p aria-live="polite">
					{#if warning.kind === 'absolute'}
						{warning.remainingMinutes > 0
							? `ช่วงการใช้งานนี้จะสิ้นสุดใน ${warning.remainingMinutes} นาที`
							: 'ช่วงการใช้งานนี้สิ้นสุดแล้ว'} กรุณาบันทึกงาน ระบบจะตรวจสอบการเข้าสู่ระบบอีกครั้งเมื่อใช้งานต่อ
						การใช้งานต่อไม่สามารถขยายเวลานี้ได้
					{:else if warning.unavailable}
						ยังต่อเวลาการใช้งานไม่ได้ กรุณาตรวจสอบการเชื่อมต่อแล้วลองอีกครั้ง
					{:else}
						{warning.remainingMinutes > 0
							? `ช่วงการใช้งานนี้จะหมดเวลาใน ${warning.remainingMinutes} นาทีหากไม่มีการใช้งาน`
							: 'ช่วงการใช้งานนี้อาจหมดเวลาแล้ว'} กรุณาบันทึกงาน ระบบจะตรวจสอบการเข้าสู่ระบบอีกครั้งเมื่อใช้งานต่อ
					{/if}
				</p>
				{#if warning.kind === 'idle'}
					<LoadingButton
						size="sm"
						loading={warning.pending}
						loadingLabel="กำลังต่อเวลา..."
						onclick={oncontinue}>ใช้งานต่อ</LoadingButton
					>
				{/if}
			</AlertDescription>
		</Alert>
	</div>
{/if}
