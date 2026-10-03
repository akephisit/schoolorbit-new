<script lang="ts">
	import { asset, resolve } from '$app/paths';
	import { goto } from '$app/navigation';
	import { authStore } from '$lib/stores/auth.svelte';
	import {
		ArrowDown,
		ArrowRight,
		ArrowUpRight,
		BadgeCheck,
		BookOpen,
		CalendarDays,
		Check,
		ClipboardList,
		GraduationCap,
		Menu,
		Orbit,
		ShieldCheck,
		Sparkles,
		UsersRound,
		X
	} from '@lucide/svelte';

	let menuOpen = $state(false);
	$effect(() => {
		if (authStore.isAuthenticated) void goto(resolve('/dashboard'));
	});

	const services = [
		{
			title: 'พื้นที่ทำงานของโรงเรียน',
			description: 'เชื่อมงานวิชาการ ตารางสอน และข้อมูลนักเรียนไว้ในพื้นที่ทำงานของแต่ละโรงเรียน',
			label: 'รู้จัก SchoolOrbit',
			route: '/#about' as const,
			icon: CalendarDays,
			tone: 'blue'
		},
		{
			title: 'เชื่อมครู นักเรียน และผู้ปกครอง',
			description: 'แต่ละคนเข้าถึงงานและข้อมูลที่เกี่ยวข้องผ่านเว็บไซต์ของโรงเรียนตนเอง',
			label: 'รู้จัก SchoolOrbit',
			route: '/#about' as const,
			icon: GraduationCap,
			tone: 'green'
		},
		{
			title: 'ดูแลระบบจากศูนย์กลาง',
			description: 'ผู้ดูแลระบบจัดการโรงเรียนและการให้บริการผ่าน SchoolOrbit Admin',
			label: 'เข้าสู่ระบบผู้ดูแล',
			route: '/login' as const,
			icon: BadgeCheck,
			tone: 'sand'
		}
	];

	const audiences = [
		{ icon: BookOpen, title: 'ครูและบุคลากร', description: 'จัดการงาน เชื่อมต่อการสอน' },
		{ icon: GraduationCap, title: 'นักเรียน', description: 'เข้าถึงทุกเรื่องของการเรียน' },
		{ icon: UsersRound, title: 'ผู้ปกครอง', description: 'ติดตามและดูแลไปด้วยกัน' }
	];
</script>

<svelte:head>
	<title>SchoolOrbit — เชื่อมทุกเรื่องของโรงเรียนไว้ในที่เดียว</title>
	<meta
		name="description"
		content="รู้จัก SchoolOrbit ระบบบริหารโรงเรียนที่เชื่อมครู นักเรียน และผู้ปกครอง พร้อมพื้นที่สำหรับผู้ดูแลระบบส่วนกลาง"
	/>
</svelte:head>

<div class="landing-page bg-[#f8fbfc] text-[#163441]">
	<a href="#main-content" class="skip-link">ข้ามไปเนื้อหาหลัก</a>
	<main id="main-content" tabindex="-1">
		<section class="landing-hero relative isolate flex min-h-screen flex-col overflow-hidden">
			<div class="hero-glow" aria-hidden="true"></div>
			<header
				class="site-header relative z-30 mx-auto mt-5 w-[calc(100%-2rem)] max-w-[1200px] sm:mt-7"
			>
				<div
					class="header-bar flex items-center justify-between gap-3 rounded-full px-4 py-3 sm:px-6"
				>
					<a
						href={resolve('/')}
						aria-label="SchoolOrbit หน้าหลัก"
						class="brand flex items-center gap-2.5"
					>
						<span
							class="flex size-10 items-center justify-center rounded-full bg-[#237c9b] text-white"
						>
							<Orbit class="size-6" strokeWidth={1.6} />
						</span>
						<span class="text-xl font-semibold tracking-tight"
							>School<span class="text-[#237c9b]">Orbit</span></span
						>
					</a>
					<nav id="main-navigation" aria-label="เมนูหลัก" class:mobile-open={menuOpen}>
						<a href="#services" onclick={() => (menuOpen = false)}>บริการ</a>
						<a href="#about" onclick={() => (menuOpen = false)}>รู้จัก SchoolOrbit</a>
						<a href="#get-started" onclick={() => (menuOpen = false)}>เริ่มต้นใช้งาน</a>
					</nav>
					<div class="flex items-center gap-1 sm:gap-3">
						<a
							href={resolve('/login')}
							class="inline-flex items-center justify-center gap-2 login-button h-10 rounded-full px-4 sm:px-5"
						>
							เข้าสู่ระบบ <ArrowUpRight class="hidden size-4 sm:block" />
						</a>
						<button
							class="flex size-10 items-center justify-center rounded-full md:hidden"
							aria-label={menuOpen ? 'ปิดเมนู' : 'เปิดเมนู'}
							aria-expanded={menuOpen}
							aria-controls="main-navigation"
							onclick={() => (menuOpen = !menuOpen)}
						>
							{#if menuOpen}<X class="size-5" />{:else}<Menu class="size-5" />{/if}
						</button>
					</div>
				</div>
			</header>

			<div
				class="hero-content relative mx-auto flex w-full max-w-[1200px] flex-1 items-center px-6"
			>
				<div class="hero-copy relative z-10 w-full max-w-[620px] py-16 lg:py-20">
					<div
						class="mb-7 inline-flex items-center gap-2.5 rounded-full border border-white/80 bg-white/65 px-4 py-2 text-sm text-[#466772]"
					>
						<span class="size-2 rounded-full bg-[#46a68a]"></span>
						พื้นที่ดิจิทัลสำหรับทุกคนในโรงเรียน
					</div>
					<h1 class="hero-title font-semibold tracking-tight">
						ทุกเรื่องของโรงเรียน<br />
						<span class="relative inline-block text-[#237c9b]"
							>เชื่อมถึงกัน<svg
								class="title-underline"
								viewBox="0 0 300 16"
								fill="none"
								aria-hidden="true"
								><path
									d="M3 11C75 1 177 0 297 8"
									stroke="currentColor"
									stroke-width="5"
									stroke-linecap="round"
								/></svg
							></span
						> ในที่เดียว
					</h1>
					<p class="mt-7 max-w-[470px] text-lg leading-relaxed text-[#54717e] sm:text-xl">
						ให้การเรียนรู้และการดูแลเดินไปด้วยกัน<br class="hidden sm:block" />
						เชื่อมงานของครู ชีวิตของนักเรียน และความใส่ใจของผู้ปกครอง ผ่าน SchoolOrbit
					</p>
					<div class="mt-9 flex flex-wrap items-center gap-4">
						<a
							href={resolve('/login')}
							class="inline-flex items-center justify-center gap-2 login-button hero-login h-14 rounded-full px-7 text-base"
						>
							เข้าสู่ระบบผู้ดูแล <span
								class="ml-2 flex size-8 items-center justify-center rounded-full bg-white/15"
								><ArrowUpRight class="size-5" /></span
							>
						</a>
						<a
							href="#services"
							class="inline-flex items-center justify-center gap-2 h-14 rounded-full px-4 text-base text-[#355a6b] hover:bg-white/60"
						>
							สำรวจบริการ <ArrowDown class="size-4" />
						</a>
					</div>
					<div class="mt-8 flex items-center gap-2 text-sm text-[#647f89]">
						<ShieldCheck class="size-4 text-[#518e82]" /> เข้าถึงข้อมูลตามสิทธิ์ของแต่ละผู้ใช้งาน
					</div>
				</div>

				<div class="campus-scene" aria-hidden="true">
					<div class="scene-orbit"></div>
					<img
						src={asset('/illustrations/school-campus.svg')}
						alt=""
						width="960"
						height="620"
						fetchpriority="high"
						class="campus-image"
					/>
					<div class="scene-card calendar-card">
						<span class="scene-icon bg-[#e5f1f6] text-[#237c9b]"
							><CalendarDays class="size-5" /></span
						>
						<div>
							<p class="text-xs text-[#6c8790]">ไม่พลาดวันสำคัญ</p>
							<p class="mt-1 font-medium">ปฏิทินโรงเรียน</p>
						</div>
						<span class="ml-3 size-2 rounded-full bg-[#66b297]"></span>
					</div>
					<div class="scene-card learning-card">
						<span class="scene-icon bg-[#edf2e6] text-[#7d9360]"><BookOpen class="size-5" /></span>
						<div>
							<p class="text-xs text-[#6c8790]">พร้อมสำหรับทุกวัน</p>
							<p class="mt-1 font-medium">ตารางเรียนและผลการเรียน</p>
						</div>
					</div>
					<div class="scene-card connected-card">
						<span
							class="flex size-7 items-center justify-center rounded-full bg-[#e4f2ec] text-[#4c9b80]"
							><Check class="size-4" /></span
						>
						<span class="text-sm">โรงเรียนที่เชื่อมถึงกัน</span>
					</div>
					<span class="scene-caption">A LITTLE MORE CONNECTED. A LOT MORE POSSIBLE.</span>
				</div>
			</div>

			<div
				class="relative z-10 mx-auto mb-8 grid w-[calc(100%-3rem)] max-w-[1152px] gap-5 rounded-2xl border border-white/90 bg-white/55 px-6 py-5 backdrop-blur-sm sm:grid-cols-3 sm:gap-8 sm:px-8"
			>
				{#each audiences as audience (audience.title)}
					<div class="flex items-center gap-4">
						<audience.icon class="size-7 shrink-0 text-[#5f8b9d]" strokeWidth={1.5} />
						<div>
							<p class="font-medium">{audience.title}</p>
							<p class="mt-1 text-sm text-[#66818b]">{audience.description}</p>
						</div>
					</div>
				{/each}
			</div>
		</section>

		<section
			id="services"
			aria-labelledby="services-heading"
			class="section-anchor mx-auto max-w-[1200px] px-6 py-20 sm:py-24"
		>
			<div class="flex flex-wrap items-end justify-between gap-5">
				<div>
					<p class="eyebrow">EXPLORE OUR SERVICES</p>
					<h2 id="services-heading" class="mt-4 text-3xl font-medium leading-snug sm:text-4xl">
						เรื่องของโรงเรียน เริ่มต้นได้ที่นี่
					</h2>
				</div>
				<p class="max-w-[300px] text-base leading-relaxed text-[#6a818b]">
					พื้นที่สำหรับทุกคนในโรงเรียน<br />และผู้ดูแลระบบส่วนกลาง
				</p>
			</div>
			<div class="mt-10 grid gap-5 md:grid-cols-3">
				{#each services as service, index (service.title)}
					<a href={resolve(service.route)} class="service-card group" data-tone={service.tone}>
						<div class="flex items-start justify-between">
							<span class="service-icon"><service.icon class="size-7" strokeWidth={1.6} /></span>
							<span class="text-sm text-[#8ba0a9]">0{index + 1}</span>
						</div>
						<h3 class="mt-8 text-2xl font-medium">{service.title}</h3>
						<p class="mt-3 flex-1 text-base leading-relaxed text-[#6a818b]">
							{service.description}
						</p>
						<div
							class="mt-8 flex items-center justify-between border-t border-[#dde7eb] pt-5 font-medium text-[#356b82]"
						>
							{service.label}<ArrowUpRight
								class="size-5 transition-transform group-hover:translate-x-1 group-hover:-translate-y-1"
							/>
						</div>
					</a>
				{/each}
			</div>
		</section>

		<section
			id="about"
			aria-labelledby="about-heading"
			class="section-anchor border-y border-[#e0ebef] bg-[#edf5f7]"
		>
			<div
				class="mx-auto grid max-w-[1200px] gap-12 px-6 py-20 sm:py-24 lg:grid-cols-[1fr_1.1fr] lg:gap-24"
			>
				<div>
					<p class="eyebrow">ONE SCHOOL. ONE ORBIT.</p>
					<h2 id="about-heading" class="mt-4 text-4xl font-medium leading-snug sm:text-5xl">
						ใกล้กันมากขึ้น<br /><span class="text-[#237c9b]">ในทุกวันของโรงเรียน</span>
					</h2>
					<p class="mt-6 max-w-[410px] text-lg leading-relaxed text-[#64808b]">
						เพราะโรงเรียนเป็นมากกว่าห้องเรียน SchoolOrbit จึงรวมงานและข้อมูลที่สำคัญ
						ให้ทุกคนใช้เวลาร่วมกันได้อย่างมีความหมาย
					</p>
					<div
						class="mt-9 inline-flex items-center gap-3 rounded-full border border-[#d1e3e8] px-4 py-2.5 text-sm text-[#4f7484]"
					>
						<Orbit class="size-5" /> เชื่อมคน เชื่อมงาน เชื่อมการเรียนรู้
					</div>
				</div>
				<div class="divide-y divide-[#d6e5ea]">
					<div class="about-feature">
						<span class="feature-icon"><ClipboardList class="size-6" strokeWidth={1.6} /></span>
						<div>
							<h3 class="text-xl font-medium">งานโรงเรียน เป็นระบบมากขึ้น</h3>
							<p class="mt-2 leading-relaxed text-[#64808b]">
								จัดการข้อมูลนักเรียน งานวิชาการ ตารางสอน และผลการเรียน ผ่านพื้นที่ทำงานเดียวกัน
							</p>
						</div>
					</div>
					<div class="about-feature">
						<span class="feature-icon"><UsersRound class="size-6" strokeWidth={1.6} /></span>
						<div>
							<h3 class="text-xl font-medium">ทุกคน มีพื้นที่ของตัวเอง</h3>
							<p class="mt-2 leading-relaxed text-[#64808b]">
								ครู นักเรียน และผู้ปกครอง เข้าถึงข้อมูลและบริการที่เกี่ยวข้องกับตัวเองได้ตามบทบาท
							</p>
						</div>
					</div>
					<div class="about-feature">
						<span class="feature-icon"><Sparkles class="size-6" strokeWidth={1.6} /></span>
						<div>
							<h3 class="text-xl font-medium">เรื่องสำคัญ อยู่ใกล้แค่ปลายนิ้ว</h3>
							<p class="mt-2 leading-relaxed text-[#64808b]">
								ติดตามกิจกรรม ตารางเรียน และเกียรติบัตร พร้อมใช้งานบนคอมพิวเตอร์และมือถือ
							</p>
						</div>
					</div>
				</div>
			</div>
		</section>

		<section
			id="get-started"
			aria-labelledby="get-started-heading"
			class="section-anchor mx-auto max-w-[1200px] px-6 py-16 sm:py-20"
		>
			<div
				class="start-panel relative isolate overflow-hidden rounded-[2rem] px-7 py-12 text-white sm:px-12 sm:py-14"
			>
				<Orbit
					class="pointer-events-none absolute -top-16 -right-14 -z-10 size-[360px] text-white/8"
					strokeWidth={0.6}
				/>
				<div class="flex flex-wrap items-center justify-between gap-8">
					<div>
						<p class="text-sm tracking-[0.12em] text-[#b9dce8]">YOUR SCHOOL, CONNECTED.</p>
						<h2 id="get-started-heading" class="mt-3 text-3xl font-medium sm:text-4xl">
							พร้อมเริ่มวันใหม่ไปด้วยกัน?
						</h2>
						<p class="mt-4 text-base text-[#c4dfe9]">
							ผู้ดูแลส่วนกลางเข้าสู่ระบบที่นี่ ครู นักเรียน
							และผู้ปกครองใช้งานผ่านเว็บไซต์ของโรงเรียน
						</p>
					</div>
					<a
						href={resolve('/login')}
						class="inline-flex items-center justify-center gap-2 h-14 rounded-full bg-white px-7 text-base text-[#236581] hover:bg-[#e8f3f7]"
						>เข้าสู่ระบบผู้ดูแล <ArrowRight class="ml-2 size-5" /></a
					>
				</div>
			</div>
		</section>
	</main>
	<footer
		class="mx-auto flex max-w-[1200px] flex-wrap items-center justify-between gap-5 border-t border-[#e0ebef] px-6 py-8 text-sm text-[#6a818b]"
	>
		<a
			href={resolve('/')}
			class="brand flex items-center gap-2 text-base font-medium text-[#355967]"
			><Orbit class="size-5 text-[#237c9b]" /> SchoolOrbit</a
		>
		<p>เชื่อมทุกเรื่องของโรงเรียนไว้ในที่เดียว</p>
		<span>SchoolOrbit Admin</span>
	</footer>
</div>

<style>
	.landing-page :global(a:focus-visible),
	.landing-page :global(button:focus-visible) {
		outline: 3px solid #237c9b;
		outline-offset: 5px;
	}
	.skip-link {
		position: fixed;
		top: 1rem;
		left: 1rem;
		z-index: 50;
		transform: translateY(-200%);
		border-radius: 0.75rem;
		background: white;
		padding: 0.75rem 1rem;
	}
	.skip-link:focus {
		transform: translateY(0);
	}
	.landing-hero {
		background: linear-gradient(115deg, #f1f6f7 0%, #e7f2f7 48%, #cde6ed 100%);
	}
	.hero-glow {
		position: absolute;
		inset: 0;
		z-index: -1;
		background:
			radial-gradient(ellipse at 90% 10%, #faf3dba8, transparent 35%),
			radial-gradient(ellipse at 20% 80%, #ffffff80, transparent 55%);
	}
	.header-bar {
		border: 1px solid #ffffffe6;
		background: #ffffffa8;
		box-shadow: 0 8px 40px #537d8e0a;
		backdrop-filter: blur(20px);
	}
	.brand {
		white-space: nowrap;
	}
	.site-header nav {
		display: flex;
		align-items: center;
		gap: 2rem;
		color: #54717e;
		font-size: 1rem;
	}
	.site-header nav a {
		transition: color 160ms;
	}
	.site-header nav a:hover {
		color: #237c9b;
	}
	.landing-page :global(.login-button) {
		background: #237c9b;
		color: white;
	}
	.landing-page :global(.login-button:hover) {
		background: #1a6683;
	}
	.landing-page :global(.hero-login) {
		box-shadow: 0 8px 24px #237c9b26;
	}
	.hero-title {
		font-size: clamp(2.8rem, 4.2vw, 4.55rem);
		line-height: 1.3;
		letter-spacing: -0.045em;
	}
	.title-underline {
		position: absolute;
		right: 0;
		bottom: -0.12em;
		left: 0;
		width: 100%;
		height: 0.23em;
		color: #8fc5d6;
	}
	.campus-scene {
		position: absolute;
		right: -150px;
		bottom: 0;
		width: 780px;
		height: 560px;
		pointer-events: none;
	}
	.campus-image {
		position: absolute;
		right: -60px;
		bottom: -15px;
		width: 900px;
		max-width: none;
		height: auto;
	}
	.scene-orbit {
		position: absolute;
		top: 6%;
		left: 14%;
		width: 70%;
		aspect-ratio: 1;
		border: 1px solid #ffffff60;
		border-radius: 50%;
		transform: rotate(-20deg) scaleX(1.2);
	}
	.scene-orbit::after {
		position: absolute;
		top: 18%;
		left: 4%;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: #97c4d2;
		content: '';
	}
	.scene-card {
		position: absolute;
		display: flex;
		align-items: center;
		gap: 12px;
		border: 1px solid #ffffffc9;
		border-radius: 16px;
		background: #ffffffbf;
		padding: 14px 17px;
		box-shadow: 0 12px 40px #44788f0f;
		backdrop-filter: blur(12px);
	}
	.scene-icon {
		display: flex;
		width: 42px;
		height: 42px;
		align-items: center;
		justify-content: center;
		border-radius: 12px;
	}
	.calendar-card {
		top: 77px;
		left: 68px;
		transform: rotate(-4deg);
		animation: float 7s ease-in-out infinite;
	}
	.learning-card {
		top: 242px;
		right: 115px;
		transform: rotate(3deg);
		animation: float 8s ease-in-out 1s infinite;
	}
	.connected-card {
		bottom: 58px;
		left: 167px;
		border-radius: 50px;
		padding: 9px 17px 9px 9px;
	}
	.scene-caption {
		position: absolute;
		right: 104px;
		bottom: 8px;
		color: #658c9a;
		font-size: 9px;
		letter-spacing: 0.16em;
	}
	.eyebrow {
		color: #6a8c9b;
		font-size: 0.8rem;
		font-weight: 500;
		letter-spacing: 0.16em;
	}
	.section-anchor {
		scroll-margin-top: 2rem;
	}
	.service-card {
		display: flex;
		flex-direction: column;
		border: 1px solid #e0e9ed;
		border-radius: 22px;
		background: white;
		padding: 28px;
		transition:
			transform 200ms,
			border-color 200ms,
			box-shadow 200ms;
	}
	.service-card:hover {
		transform: translateY(-5px);
		border-color: #b4d5e0;
		box-shadow: 0 14px 40px #3c718a0b;
	}
	.service-icon {
		display: flex;
		width: 58px;
		height: 58px;
		align-items: center;
		justify-content: center;
		border-radius: 18px;
		background: #e9f3f8;
		color: #4388a5;
	}
	.service-card[data-tone='green'] .service-icon {
		background: #e9f3ed;
		color: #6d9b80;
	}
	.service-card[data-tone='sand'] .service-icon {
		background: #f7f1e5;
		color: #b99a5e;
	}
	.about-feature {
		display: flex;
		gap: 22px;
		padding: 26px 0;
	}
	.about-feature:first-child {
		padding-top: 0;
	}
	.about-feature:last-child {
		padding-bottom: 0;
	}
	.feature-icon {
		display: flex;
		width: 48px;
		height: 48px;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		border: 1px solid #d4e5eb;
		border-radius: 15px;
		color: #578b9f;
	}
	.start-panel {
		background: radial-gradient(ellipse at 90% 100%, #348cab, transparent 70%), #235e77;
	}
	@keyframes float {
		0%,
		100% {
			translate: 0 0;
		}
		50% {
			translate: 0 -8px;
		}
	}
	@media (min-width: 1600px) {
		.campus-scene {
			right: -220px;
			width: 900px;
			height: 610px;
		}
		.campus-image {
			width: 1020px;
		}
		.calendar-card {
			top: 90px;
			left: 90px;
		}
		.learning-card {
			right: 100px;
		}
	}
	@media (max-width: 1100px) and (min-width: 768px) {
		.hero-copy {
			max-width: 540px;
		}
		.hero-title {
			font-size: 3.65rem;
		}
		.campus-scene {
			right: -270px;
			opacity: 0.75;
		}
		.scene-card {
			display: none;
		}
		.hero-copy::before {
			position: absolute;
			inset: 0 -50px 0 -24px;
			z-index: -1;
			background: linear-gradient(90deg, #eaf3f7 65%, transparent);
			content: '';
		}
	}
	@media (max-width: 767px) {
		.site-header nav {
			position: absolute;
			top: calc(100% + 10px);
			right: 0;
			left: 0;
			display: none;
			align-items: stretch;
			gap: 0;
			border: 1px solid #dce9ee;
			border-radius: 20px;
			background: #f8fcfd;
			padding: 10px;
			box-shadow: 0 12px 30px #355b701a;
		}
		.site-header nav.mobile-open {
			display: flex;
			flex-direction: column;
		}
		.site-header nav a {
			border-radius: 12px;
			padding: 12px 16px;
		}
		.site-header nav a:hover {
			background: #eaf4f8;
		}
		.hero-content {
			flex-direction: column;
			align-items: stretch;
		}
		.hero-copy {
			max-width: none;
			padding-top: 55px;
			padding-bottom: 0;
		}
		.hero-title {
			font-size: clamp(2.65rem, 7.8vw, 4rem);
		}
		.campus-scene {
			position: relative;
			right: auto;
			bottom: auto;
			left: 50%;
			width: 500px;
			height: 330px;
			margin-top: 28px;
			margin-bottom: 12px;
			transform: translateX(-50%);
		}
		.campus-image {
			right: -25px;
			bottom: -22px;
			width: 555px;
		}
		.scene-orbit {
			top: 0;
		}
		.scene-card {
			gap: 8px;
			border-radius: 12px;
			padding: 10px 12px;
		}
		.scene-icon {
			width: 34px;
			height: 34px;
			border-radius: 9px;
		}
		.calendar-card {
			top: 12px;
			left: 74px;
		}
		.learning-card {
			top: 132px;
			right: 85px;
			font-size: 12px;
		}
		.connected-card {
			bottom: 19px;
			left: 100px;
			border-radius: 50px;
			padding: 7px 12px 7px 7px;
		}
		.scene-caption {
			display: none;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.scene-card {
			animation: none;
		}
		.service-card,
		.site-header nav a {
			transition: none;
		}
		.service-card:hover {
			transform: none;
		}
		.landing-page :global(.transition-transform) {
			transition: none;
		}
	}
</style>
