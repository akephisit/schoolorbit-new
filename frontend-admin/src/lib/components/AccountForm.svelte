<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { apiClient } from '#lib/api/client.js';
	import { authStore } from '#lib/stores/auth.svelte.js';
	import { Orbit, ArrowRight, MailCheck } from '@lucide/svelte';

	let { mode }: { mode: 'login' | 'register' | 'verify' | 'forgot' | 'reset' | 'resend' } =
		$props();
	let mounted = $state(false);
	let email = $state('');
	let password = $state('');
	let name = $state('');
	let token = $state('');
	let busy = $state(false);
	let error = $state('');
	let success = $state('');
	const titles = {
		login: 'ยินดีต้อนรับกลับ',
		register: 'เริ่มต้นกับ SchoolOrbit',
		verify: 'ยืนยันอีเมลของคุณ',
		forgot: 'ลืมรหัสผ่าน',
		reset: 'ตั้งรหัสผ่านใหม่',
		resend: 'ส่งอีเมลยืนยันอีกครั้ง'
	};
	const labels = {
		login: 'เข้าสู่ระบบ',
		register: 'สร้างบัญชี',
		verify: 'ยืนยันอีเมล',
		forgot: 'ส่งลิงก์ตั้งรหัสผ่าน',
		reset: 'บันทึกรหัสผ่านใหม่',
		resend: 'ส่งอีเมลยืนยัน'
	};
	onMount(() => {
		mounted = true;
		token = new URLSearchParams(window.location.hash.slice(1)).get('token') || '';
		if (token) history.replaceState(null, '', window.location.pathname);
	});
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = '';
		success = '';
		try {
			if (mode === 'login') {
				const result = await authStore.login({ email, password });
				if (!result.success)
					error = result.error || 'อีเมลหรือรหัสผ่านไม่ถูกต้อง หรือยังไม่ได้ยืนยันอีเมล';
				return;
			}
			const result =
				mode === 'register'
					? await apiClient.register({ email, password, name })
					: mode === 'verify'
						? await apiClient.verify(token)
						: mode === 'reset'
							? await apiClient.resetPassword(token, password)
							: await apiClient.emailAction(
									mode === 'forgot' ? 'forgot-password' : 'resend-verification',
									email
								);
			if (!result.success) {
				error = result.error || 'ไม่สามารถดำเนินการได้ กรุณาลองอีกครั้ง';
				return;
			}
			success =
				mode === 'verify'
					? 'ยืนยันอีเมลแล้ว คุณสามารถเข้าสู่ระบบได้เลย'
					: mode === 'reset'
						? 'บันทึกรหัสผ่านแล้ว กรุณาเข้าสู่ระบบใหม่'
						: 'หากข้อมูลตรงกับบัญชี ระบบจะส่งอีเมลให้ กรุณาตรวจสอบกล่องจดหมายและจดหมายขยะ';
			password = '';
			if (mode === 'verify' || mode === 'reset') token = '';
		} finally {
			busy = false;
		}
	}
</script>

<svelte:head
	><title>{titles[mode]} — SchoolOrbit</title><meta name="robots" content="noindex" /><meta
		name="referrer"
		content="no-referrer"
	/></svelte:head
>
<main class="account-page">
	<div class="account-intro">
		<a class="portal-brand" href={resolve('/')}><Orbit size={32} /> SchoolOrbit</a>
		<span class="portal-eyebrow">พื้นที่สำหรับผู้ดูแลบริการโรงเรียน</span>
		<h1>เชื่อมทุกเรื่อง<br />ของโรงเรียนไว้ด้วยกัน</h1>
		<p>ดูแลโรงเรียน แพ็กเกจ และการต่ออายุ<br />ผ่านบัญชีเดียวของคุณ</p>
		<img src="/illustrations/school-campus.svg" alt="" width="960" height="620" />
	</div>
	<section class="account-card" aria-labelledby="account-title">
		<span class="portal-eyebrow">บัญชี SchoolOrbit</span>
		<h2 id="account-title">{titles[mode]}</h2>
		<p class="portal-muted">
			{mode === 'register'
				? 'สมัครด้วยอีเมลและยืนยันก่อนเริ่มจัดการบริการ'
				: mode === 'verify' || mode === 'reset'
					? 'ลิงก์นี้ใช้ได้ครั้งเดียว กรุณากดปุ่มเพื่อดำเนินการ'
					: 'จัดการบริการโรงเรียนของคุณได้จากที่นี่'}
		</p>
		{#if success}<div class="portal-notice" role="status">
				<MailCheck size={20} />{success}
			</div>{/if}
		{#if error}<p class="portal-error" role="alert">{error}</p>{/if}
		{#if !success || mode === 'resend' || mode === 'forgot'}
			<form method="POST" onsubmit={submit} class="portal-form" aria-busy={busy}>
				{#if mode === 'register'}<label
						>ชื่อผู้ติดต่อ<input
							bind:value={name}
							autocomplete="name"
							required
							maxlength="120"
							disabled={busy || !mounted}
						/></label
					>{/if}
				{#if mode !== 'verify' && mode !== 'reset'}<label
						>อีเมล<input
							type="email"
							bind:value={email}
							autocomplete="email"
							required
							maxlength="254"
							placeholder="you@school.ac.th"
							disabled={busy || !mounted}
						/></label
					>{/if}
				{#if mode === 'login' || mode === 'register' || mode === 'reset'}<label
						>รหัสผ่าน<input
							type="password"
							bind:value={password}
							autocomplete={mode === 'login' ? 'current-password' : 'new-password'}
							required
							minlength="8"
							disabled={busy || !mounted}
						/></label
					>{/if}
				{#if (mode === 'verify' || mode === 'reset') && !token}<p class="portal-error">
						ไม่พบลิงก์ที่ใช้งานได้ กรุณาขออีเมลใหม่
					</p>{/if}
				<button
					class="portal-button"
					disabled={!mounted || busy || ((mode === 'verify' || mode === 'reset') && !token)}
					type="submit">{busy ? 'กำลังดำเนินการ…' : labels[mode]}<ArrowRight size={18} /></button
				>
			</form>
		{/if}
		<nav class="account-links" aria-label="บัญชีผู้ใช้">
			{#if mode !== 'login'}<a href={resolve('/login')}>เข้าสู่ระบบ</a>{/if}
			{#if mode !== 'register'}<a href={resolve('/register')}>สร้างบัญชีใหม่</a>{/if}
			{#if mode === 'login'}<a href={resolve('/forgot-password')}>ลืมรหัสผ่าน</a>{/if}
			{#if mode === 'login' || mode === 'verify' || mode === 'register'}<a
					href={resolve('/resend-verification')}>ส่งอีเมลยืนยันอีกครั้ง</a
				>{/if}
			{#if mode === 'reset'}<a href={resolve('/forgot-password')}>ขอลิงก์ใหม่</a>{/if}
		</nav>
	</section>
</main>
