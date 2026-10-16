<script lang="ts">
	import { apiClient, type ServiceRequestInput } from '#lib/api/client.js';
	import { authStore } from '#lib/stores/auth.svelte.js';
	import { invalidateAll } from '$app/navigation';
	import ServiceDecision from '#lib/components/ServiceDecision.svelte';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	let operator = $derived(authStore.user?.role === 'operator');
	let kind = $state<'open_school' | 'renew_school'>('open_school');
	let cadence = $state<'monthly' | 'yearly'>('monthly');
	let schoolName = $state('');
	let subdomain = $state('');
	let schoolId = $state('');
	let requestId = $state(crypto.randomUUID());
	let busy = $state(false);
	let uncertain = $state(false);
	let error = $state('');
	let success = $state('');
	let assignEmail = $state('');
	let assignSchool = $state('');
	const statuses = { pending: 'รอตรวจสอบ', approved: 'อนุมัติแล้ว', rejected: 'ไม่อนุมัติ' };
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = '';
		success = '';
		const input: ServiceRequestInput = {
			id: requestId,
			kind,
			cadence,
			schoolId: kind === 'renew_school' ? schoolId : null,
			schoolName,
			subdomain
		};
		const result = await apiClient.createServiceRequest(input);
		if (result.success) {
			success = 'ส่งคำขอแล้ว ทีมงานจะตรวจสอบและติดต่อกลับ';
			requestId = crypto.randomUUID();
		} else {
			error = result.error || 'ส่งคำขอไม่สำเร็จ กรุณาตรวจสอบรายการก่อนส่งใหม่';
			uncertain = true;
		}
		await invalidateAll();
		busy = false;
	}
	async function reconcile() {
		busy = true;
		await invalidateAll();
		if (data.requests.data?.some((request) => request.id === requestId)) {
			success = 'พบคำขอที่บันทึกแล้ว';
			requestId = crypto.randomUUID();
		}
		uncertain = false;
		busy = false;
	}
	async function assign(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = '';
		success = '';
		const result = await apiClient.assignSchool(assignSchool, assignEmail);
		if (result.success) success = 'เชื่อมโรงเรียนกับบัญชีที่ยืนยันอีเมลแล้ว';
		else error = result.error || 'ไม่สามารถเชื่อมโรงเรียนได้';
		busy = false;
	}
</script>

<svelte:head><title>คำขอรับบริการ — SchoolOrbit</title></svelte:head>
<h1 class="portal-title">{operator ? 'คำขอจากลูกค้า' : 'ขอเปิดโรงเรียนและต่ออายุ'}</h1>
<p class="portal-muted">ทีมงานตรวจสอบและอนุมัติบริการในระยะแรก ยังไม่มีการตัดเงินอัตโนมัติ</p>
{#if success}<p class="portal-notice" role="status">{success}</p>{/if}
{#if error}<p class="portal-error" role="alert">{error}</p>{/if}
{#if !operator}
	<section class="portal-card mt-6">
		<h2>ส่งคำขอรับบริการ</h2>
		<form class="portal-form" onsubmit={submit}>
			<label
				>รายการ<select bind:value={kind} disabled={busy || uncertain}
					><option value="open_school">ขอเปิดโรงเรียน</option><option value="renew_school"
						>ขอต่ออายุ</option
					></select
				></label
			>
			{#if kind === 'open_school'}<label
					>ชื่อโรงเรียน<input
						bind:value={schoolName}
						required
						maxlength="255"
						disabled={busy || uncertain}
					/></label
				><label
					>รหัสเว็บไซต์โรงเรียน<input
						bind:value={subdomain}
						required
						pattern="[a-z0-9][a-z0-9-]*"
						maxlength="50"
						placeholder="ชื่อภาษาอังกฤษ เช่น my-school"
						disabled={busy || uncertain}
					/></label
				>
			{:else}<label
					>โรงเรียน<select bind:value={schoolId} required disabled={busy || uncertain}
						><option value="">เลือกโรงเรียนของคุณ</option
						>{#each data.schools.data || [] as school (school.id)}<option value={school.id}
								>{school.name}</option
							>{/each}</select
					></label
				>{/if}
			<label
				>รอบบริการ<select bind:value={cadence} disabled={busy || uncertain}
					><option value="monthly">รายเดือน</option><option value="yearly">รายปี</option></select
				></label
			>
			<div class="portal-notice">
				เมื่อแพ็กเกจหมดอายุ ระบบจะแจ้งเตือน โรงเรียนยังใช้งานได้ตามปกติ
			</div>
			<button class="portal-button" type="submit" disabled={busy || uncertain}
				>{busy ? 'กำลังส่ง…' : 'ส่งคำขอให้ทีมงานตรวจสอบ'}</button
			>
			{#if uncertain}<button
					class="portal-button secondary"
					type="button"
					onclick={reconcile}
					disabled={busy}>ตรวจสอบรายการก่อนส่งใหม่</button
				>{/if}
		</form>
	</section>
{:else}
	<section class="portal-card mt-6">
		<h2>เชื่อมโรงเรียนเดิมกับผู้ดูแลบริการ</h2>
		<p class="portal-muted">
			ตรวจสอบเจ้าของก่อนดำเนินการ บัญชีต้องเป็นลูกค้าที่ผ่านการยืนยันอีเมลแล้ว
		</p>
		{#if data.managed && !data.managed.success}<p class="portal-error">{data.managed.error}</p>{/if}
		<form class="portal-form" onsubmit={assign}>
			<label
				>โรงเรียน<select bind:value={assignSchool} required disabled={busy}
					><option value="">เลือกโรงเรียน</option
					>{#each data.managed?.data?.schools || [] as school (school.id)}<option value={school.id}
							>{school.name} ({school.subdomain})</option
						>{/each}</select
				></label
			><label
				>อีเมลผู้ดูแล<input type="email" bind:value={assignEmail} required disabled={busy} /></label
			><button class="portal-button" disabled={busy} type="submit">เชื่อมบัญชี</button>
		</form>
	</section>
{/if}
<h2 class="mt-8 text-xl">{operator ? 'รายการคำขอ' : 'ประวัติคำขอของคุณ'}</h2>
{#if !data.requests.success}<div class="portal-error" role="alert">
		{data.requests.error}<button class="portal-button secondary" onclick={() => invalidateAll()}
			>ลองอีกครั้ง</button
		>
	</div>
{:else if !data.requests.data?.length}<p class="portal-muted mt-4">ยังไม่มีคำขอรับบริการ</p>
{:else}<div class="portal-grid">
		{#each data.requests.data as request (request.id)}<article class="portal-card">
				<span class="portal-badge">{statuses[request.status]}</span>
				<h3 class="mt-3">{request.schoolName}</h3>
				<p class="portal-muted">
					{request.subdomain} · {request.kind === 'open_school' ? 'เปิดโรงเรียน' : 'ต่ออายุ'} · {request.cadence ===
					'monthly'
						? 'รายเดือน'
						: 'รายปี'}
				</p>
				{#if operator}<p>{request.email}</p>{/if}{#if request.decisionReason}<p class="mt-3">
						ผลพิจารณา: {request.decisionReason}
					</p>{/if}{#if operator && request.status === 'pending'}<ServiceDecision
						{request}
						schools={data.managed?.data?.schools || []}
					/>{/if}
			</article>{/each}
	</div>{/if}
