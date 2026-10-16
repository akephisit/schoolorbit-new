<script lang="ts">
	import { resolve } from '$app/paths';
	import { invalidateAll } from '$app/navigation';
	import { authStore } from '#lib/stores/auth.svelte.js';
	import type { PageProps } from './$types';
	let { data }: PageProps = $props();
	const date = (value: string) =>
		new Intl.DateTimeFormat('th-TH', { dateStyle: 'long', timeZone: 'Asia/Bangkok' }).format(
			new Date(value)
		);
</script>

<svelte:head><title>โรงเรียนของฉัน — SchoolOrbit</title></svelte:head>
<span class="portal-eyebrow">พื้นที่บริการของคุณ</span>
<h1 class="portal-title">โรงเรียนของฉัน</h1>
<p class="portal-muted">ดูแลบริการและติดตามวันต่ออายุของโรงเรียนได้ในที่เดียว</p>
<div class="portal-actions">
	<a class="portal-button" href={resolve('/dashboard/requests')}>ขอเปิดโรงเรียน / ต่ออายุ</a
	>{#if authStore.user?.role === 'operator'}<a
			class="portal-button secondary"
			href={resolve('/dashboard/schools')}>จัดการโรงเรียนทั้งหมด</a
		>{/if}
</div>
{#if !data.schools.success}<div class="portal-error" role="alert">
		{data.schools.error}<button class="portal-button secondary" onclick={() => invalidateAll()}
			>ลองอีกครั้ง</button
		>
	</div>
{:else if !data.schools.data?.length}<div class="portal-card mt-6">
		<h2>ยังไม่มีโรงเรียนที่เชื่อมกับบัญชีนี้</h2>
		<p class="portal-muted">
			ส่งคำขอเปิดบริการได้เลย หากเป็นโรงเรียนเดิม ทีมงานจะตรวจสอบและเชื่อมให้
			โดยข้อมูลโรงเรียนยังคงอยู่
		</p>
	</div>
{:else}<div class="portal-grid">
		{#each data.schools.data as school (school.id)}<article class="portal-card">
				<h2>{school.name}</h2>
				<p class="portal-muted">{school.subdomain}</p>
				<p>
					{school.cadence === 'monthly'
						? 'รายเดือน'
						: school.cadence === 'yearly'
							? 'รายปี'
							: 'ยังไม่ได้กำหนดแพ็กเกจ'}
				</p>
				{#if school.endsAt}<p>วันหมดอายุ: {date(school.endsAt)}</p>{/if}{#if school.expired}<div
						class="portal-notice"
					>
						แพ็กเกจหมดอายุแล้ว กรุณาติดต่อเพื่อต่ออายุ โรงเรียนยังใช้งานได้ตามปกติ
					</div>{/if}
			</article>{/each}
	</div>{/if}
