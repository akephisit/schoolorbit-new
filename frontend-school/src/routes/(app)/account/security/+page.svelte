<script lang="ts">
	import { PageShell } from '$lib/components/app-layout';
	import SessionSecurityPanel from '$lib/features/session-security/SessionSecurityPanel.svelte';
</script>

<PageShell title="ความปลอดภัยของบัญชี" description="จัดการรหัสผ่านและอุปกรณ์ที่เข้าสู่ระบบ">
	<SessionSecurityPanel />
</PageShell>
