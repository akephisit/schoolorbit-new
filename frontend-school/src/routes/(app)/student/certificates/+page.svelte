<script lang="ts">
	import type { PageProps } from './$types';
	import { authStore } from '$lib/stores/auth';
	import { can } from '$lib/stores/permissions';
	import { PERMISSIONS } from '$lib/permissions/registry';
	let { data }: PageProps = $props();
	import MyCertificateList from '$lib/components/certificates/MyCertificateList.svelte';
</script>

<MyCertificateList
	initialCertificates={data.certificates}
	userId={$authStore.user?.id ?? ''}
	canRead={$can.has(PERMISSIONS.CERTIFICATE_READ_OWN)}
	title="เกียรติบัตรของฉัน"
	description="เกียรติบัตรที่โรงเรียนออกและเชื่อมกับบัญชีนักเรียนของคุณ"
/>
