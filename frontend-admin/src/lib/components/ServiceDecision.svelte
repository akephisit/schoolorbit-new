<script lang="ts">
	import { apiClient, type ServiceRequest, type School } from '#lib/api/client.js';
	import { invalidateAll } from '$app/navigation';
	let { request, schools }: { request: ServiceRequest; schools: School[] } = $props();
	let schoolId = $state('');
	let reason = $state('');
	let busy = $state(false);
	let error = $state('');
	let matches = $derived(schools.filter((school) => school.subdomain === request.subdomain));
	async function decide(approve: boolean) {
		busy = true;
		error = '';
		const result = await apiClient.decideRequest(request.id, {
			approve,
			schoolId: request.schoolId || schoolId || null,
			reason
		});
		if (!result.success) error = result.error || 'ไม่สามารถบันทึกผลได้ กรุณาตรวจสอบรายการใหม่';
		await invalidateAll();
		busy = false;
	}
</script>

<div class="portal-form">
	{#if request.kind === 'open_school'}<label
			>โรงเรียนที่สร้างและตรวจสอบแล้ว<select bind:value={schoolId} disabled={busy}
				><option value="">เลือกโรงเรียน</option>{#each matches as school (school.id)}<option
						value={school.id}>{school.name} ({school.subdomain})</option
					>{/each}</select
			></label
		>
		<p class="portal-muted">
			หากยังไม่มีโรงเรียน ให้สร้างจากเมนูจัดการโรงเรียนก่อน การอนุมัติจะเชื่อมบัญชีและเริ่มรอบบริการ
		</p>{/if}
	<label
		>เหตุผล / หลักฐานการอนุมัติ<textarea
			bind:value={reason}
			required
			maxlength="500"
			disabled={busy}></textarea></label
	>
	{#if error}<p class="portal-error" role="alert">{error}</p>{/if}
	<div class="portal-actions">
		<button
			class="portal-button"
			disabled={busy || !reason.trim() || (request.kind === 'open_school' && !schoolId)}
			onclick={() => decide(true)}>{busy ? 'กำลังบันทึก…' : 'อนุมัติและเริ่มรอบบริการ'}</button
		><button
			class="portal-button secondary"
			disabled={busy || !reason.trim()}
			onclick={() => decide(false)}>ไม่อนุมัติ</button
		>
	</div>
</div>
