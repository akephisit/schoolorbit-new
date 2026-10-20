<script lang="ts">
	import { can } from '#lib/stores/permissions.js';
	import { budgetReadPermissions } from '#lib/budget/access.js';
	import { PageState } from '#lib/components/app-state/index.js';
</script>

{#if !$can.hasAny(...budgetReadPermissions)}<PageState
		variant="permission"
		title="ต้องมีสิทธิ์ดูงบประมาณ"
		description="ขอสิทธิ์ดูงบประมาณในขอบเขตงานของคุณจากผู้ดูแลก่อนสร้างโครงการหรือทำรายการการเงิน"
	/>{/if}
