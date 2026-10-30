<script lang="ts">
	import type { SessionWarning } from '#lib/auth/session-activity.js';
	import { Alert, AlertDescription, AlertTitle } from '#lib/components/ui/alert/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	let {
		warning,
		oncontinue
	}: { warning: SessionWarning | null; oncontinue: (event: MouseEvent) => void } = $props();
</script>

{#if warning}
	<div class="px-4 pt-3 sm:px-6" data-testid="session-warning">
		<Alert class="border-warning/40 bg-warning/10">
			<AlertTitle>การเข้าสู่ระบบใกล้หมดเวลา</AlertTitle>
			<AlertDescription class="flex flex-wrap items-center justify-between gap-3">
				<p aria-live="polite">
					{#if warning.kind === 'absolute'}
						ครบระยะเวลาการเข้าสู่ระบบ{warning.remainingMinutes > 0
							? `ใน ${warning.remainingMinutes} นาที`
							: 'แล้ว'} กรุณาบันทึกงานและเข้าสู่ระบบอีกครั้ง การใช้งานต่อไม่สามารถขยายเวลานี้ได้
					{:else if warning.unavailable}
						ยังต่อเวลาการใช้งานไม่ได้ กรุณาตรวจสอบการเชื่อมต่อแล้วลองอีกครั้ง
					{:else}
						{warning.remainingMinutes > 0
							? `หากไม่มีการใช้งาน คุณจะออกจากระบบใน ${warning.remainingMinutes} นาที`
							: 'การใช้งานอาจหมดเวลาแล้ว กรุณาตรวจสอบก่อนทำงานต่อ'}
					{/if}
				</p>
				{#if warning.kind === 'idle'}
					<Button size="sm" disabled={warning.pending} onclick={oncontinue}>ใช้งานต่อ</Button>
				{/if}
			</AlertDescription>
		</Alert>
	</div>
{/if}
