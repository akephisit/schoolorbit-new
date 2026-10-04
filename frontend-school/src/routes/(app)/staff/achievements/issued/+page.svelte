<script lang="ts">
	import type { PageProps } from './$types';
	import { authStore } from '#lib/stores/auth.js';
	import { can } from '#lib/stores/permissions.js';
	import { PERMISSIONS } from '#lib/permissions/registry.js';
	let { data }: PageProps = $props();
	import MyCertificateList from '#lib/components/certificates/MyCertificateList.svelte';
</script>

<MyCertificateList
	initialCertificates={data.certificates}
	userId={$authStore.user?.id ?? ''}
	canRead={$can.has(PERMISSIONS.CERTIFICATE_READ_OWN)}
	title="เกียรติบัตรที่โรงเรียนออก"
	description="ใบที่ผูกกับบัญชีบุคลากรของคุณ แยกจากผลงานที่บันทึกด้วยตนเอง"
/>
