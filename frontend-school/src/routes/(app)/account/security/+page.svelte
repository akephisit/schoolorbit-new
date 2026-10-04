<script lang="ts">
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	import { PageShell } from '#lib/components/app-layout/index.js';
	import SessionSecurityPanel from '#lib/features/session-security/SessionSecurityPanel.svelte';
</script>

<PageShell title="ความปลอดภัยของบัญชี" description="จัดการรหัสผ่านและอุปกรณ์ที่เข้าสู่ระบบ">
	<SessionSecurityPanel source={data.sessions} requestKey={data.requestKey} />
</PageShell>
