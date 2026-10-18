<script lang="ts">
	import type { SchoolAppearance, PreviewMode } from '#lib/appearance/model.js';
	import { appearanceStyle } from '#lib/appearance/model.js';

	import PreviewShell from './PreviewShell.svelte';
	import { Button, buttonVariants } from '#lib/components/ui/button/index.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import {
		Card,
		CardContent,
		CardHeader,
		CardTitle,
		CardDescription
	} from '#lib/components/ui/card/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Sheet from '#lib/components/ui/sheet/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import {
		ArrowRight,
		Bell,
		BookOpen,
		Building2,
		CalendarDays,
		Check,
		ChevronDown,
		ChevronLeft,
		CircleAlert,
		ClipboardCheck,
		Clock3,
		FileText,
		GraduationCap,
		HeartHandshake,
		Home,
		Inbox,
		LayoutGrid,
		Menu,
		Moon,
		PanelLeftClose,
		PanelLeftOpen,
		School,
		ShieldCheck,
		Sun,
		Users,
		Wallet
	} from '@lucide/svelte';

	let { appearance, mode = 'light' }: { appearance: SchoolAppearance; mode?: PreviewMode } =
		$props();
	let dark = $derived(mode === 'dark');
	const compactDensity = $derived(appearance.density === 'compact');
	const previewStyle = $derived(appearanceStyle(appearance, dark ? 'dark' : 'light'));
	let collapsed = $state(false);
	let mobileOpen = $state(false);
	let demoOpen = $state(false);
	let demoTitle = $state('');
	let year = $state('2569');
	let term = $state('1');
	let closedSections = $state<string[]>([]);

	const workspaces = [
		{
			name: 'กลุ่มบริหารวิชาการ',
			icon: BookOpen,
			sections: [
				{
					name: 'งานจัดการเรียนรู้',
					icon: GraduationCap,
					items: [
						{ name: 'ตารางสอน', icon: CalendarDays },
						{ name: 'บันทึกผลการเรียน', icon: ClipboardCheck }
					]
				},
				{
					name: 'งานทะเบียนและวัดผล',
					icon: FileText,
					items: [
						{ name: 'ข้อมูลนักเรียน', icon: Users },
						{ name: 'ทะเบียนรายวิชา', icon: BookOpen }
					]
				}
			]
		},
		{
			name: 'กลุ่มบริหารทั่วไป',
			icon: Building2,
			sections: [
				{
					name: 'งานบริหารสำนักงาน',
					icon: Building2,
					items: [
						{ name: 'สารบรรณ', icon: FileText },
						{ name: 'พัสดุและครุภัณฑ์', icon: Wallet }
					]
				}
			]
		},
		{
			name: 'กลุ่มบริหารกิจการนักเรียน',
			icon: HeartHandshake,
			sections: [
				{
					name: 'งานดูแลช่วยเหลือนักเรียน',
					icon: ShieldCheck,
					items: [
						{ name: 'เช็กชื่อนักเรียน', icon: ClipboardCheck },
						{ name: 'ระบบดูแลช่วยเหลือ', icon: HeartHandshake }
					]
				}
			]
		}
	];
	const serviceCount = workspaces.reduce(
		(total, workspace) =>
			total + workspace.sections.reduce((count, section) => count + section.items.length, 0),
		1
	);
	const summaries = [
		{
			label: 'งานที่เปิดอยู่',
			value: '12',
			icon: Inbox,
			tone: 'primary',
			note: 'รายการที่กำลังดำเนินการ'
		},
		{
			label: 'ใกล้ครบกำหนด',
			value: '3',
			icon: Clock3,
			tone: 'warning',
			note: 'เตรียมดำเนินการให้ทันเวลา'
		},
		{
			label: 'เกินกำหนด',
			value: '1',
			icon: CircleAlert,
			tone: 'destructive',
			note: 'รายการที่ต้องติดตาม'
		},
		{
			label: 'บริการที่เข้าถึงได้',
			value: String(serviceCount),
			icon: LayoutGrid,
			tone: 'primary',
			note: 'ตามสิทธิ์การใช้งานของคุณ'
		}
	];
	const overview = [
		{ label: 'บุคลากรทั้งหมด', value: '86', unit: 'คน', icon: Users },
		{ label: 'นักเรียนทั้งหมด', value: '1,248', unit: 'คน', icon: GraduationCap },
		{ label: 'ห้องเรียนที่เปิด', value: '36', unit: 'ห้อง', icon: Building2 }
	];

	function showDemo(title: string) {
		demoTitle = title;
		mobileOpen = false;
		demoOpen = true;
	}
	function toggleSection(name: string) {
		closedSections = closedSections.includes(name)
			? closedSections.filter((item) => item !== name)
			: [...closedSections, name];
	}
</script>

<div
	class:dark
	class:collapsed
	class="design-preview"
	class:compact-density={compactDensity}
	style={previewStyle}
	data-testid="staff-design-preview"
>
	{#snippet navigation(compact: boolean)}
		<div class="brand">
			<span class="brand-mark"><GraduationCap class="size-6" /></span>
			{#if !compact}<div>
					<strong>SchoolOrbit<span class="brand-dot">.</span></strong>
					<p>พื้นที่ทำงานของโรงเรียน</p>
				</div>{/if}
		</div>
		{#if !compact}
			<div class="school-identity">
				<span class="school-mark"><School class="size-5" /></span>
				<div>
					<strong>โรงเรียนแสงปัญญา</strong>
					<p>พื้นที่ตัวอย่างสำหรับบุคลากร</p>
				</div>
			</div>
		{/if}
		<nav aria-label="เมนูหลักตัวอย่าง" class="navigation">
			{#if !compact}<p class="nav-eyebrow">พื้นที่ทำงานของฉัน</p>{/if}
			<Button
				variant="ghost"
				class="nav-link active"
				aria-current="page"
				aria-label="หน้าหลักของฉัน"
				onclick={() => {
					mobileOpen = false;
				}}
				><Home class="size-4 shrink-0" />{#if !compact}<span>หน้าหลักของฉัน</span><span
						class="active-dot"
					></span>{/if}</Button
			>
			<Button
				variant="ghost"
				class="nav-link"
				aria-label="งานของฉัน"
				onclick={() => showDemo('งานของฉัน')}
				><Inbox class="size-4 shrink-0" />{#if !compact}<span>งานของฉัน</span><Badge
						variant="secondary"
						class="ml-auto">12</Badge
					>{/if}</Button
			>
			{#each workspaces as workspace (workspace.name)}
				<div class="nav-workspace">
					{#if compact}
						<DropdownMenu.Root>
							<DropdownMenu.Trigger
								class={buttonVariants({ variant: 'ghost', size: 'icon' })}
								aria-label={workspace.name}><workspace.icon class="size-5" /></DropdownMenu.Trigger
							>
							<DropdownMenu.Content
								side="right"
								align="start"
								portalProps={{ disabled: true }}
								class="w-64"
							>
								<DropdownMenu.Label>{workspace.name}</DropdownMenu.Label>
								<DropdownMenu.Separator />
								{#each workspace.sections as section (section.name)}
									<DropdownMenu.Group>
										<DropdownMenu.GroupHeading class="text-sm text-muted-foreground"
											>{section.name}</DropdownMenu.GroupHeading
										>
										{#each section.items as item (item.name)}<DropdownMenu.Item
												onSelect={() => showDemo(item.name)}
												><item.icon class="size-4" />{item.name}</DropdownMenu.Item
											>{/each}
									</DropdownMenu.Group>
								{/each}
							</DropdownMenu.Content>
						</DropdownMenu.Root>
					{:else}
						<p class="workspace-name"><workspace.icon class="size-3.5" />{workspace.name}</p>
						{#each workspace.sections as section (section.name)}
							<Button
								variant="ghost"
								class="section-toggle"
								aria-expanded={!closedSections.includes(section.name)}
								onclick={() => toggleSection(section.name)}
								><span>{section.name}</span><ChevronDown
									class={`size-3.5 transition-transform ${closedSections.includes(section.name) ? '-rotate-90' : ''}`}
								/></Button
							>
							{#if !closedSections.includes(section.name)}
								<div class="nav-items">
									{#each section.items as item (item.name)}<Button
											variant="ghost"
											class="nav-link"
											onclick={() => showDemo(item.name)}
											><item.icon class="size-4 shrink-0" /><span>{item.name}</span></Button
										>{/each}
								</div>
							{/if}
						{/each}
					{/if}
				</div>
			{/each}
		</nav>
		<div class="sidebar-footer">
			{#if !compact}<div class="connection"><span></span>พื้นที่สาธิตดีไซน์</div>
				<p>SchoolOrbit · Staff workspace</p>{:else}<GraduationCap
					class="size-5 text-muted-foreground"
				/>{/if}
		</div>
	{/snippet}

	<aside class="desktop-sidebar" aria-label="แถบเมนูตัวอย่าง">
		{@render navigation(collapsed)}
		<Button
			variant="outline"
			size="icon"
			class="collapse-control"
			aria-label={collapsed ? 'ขยายแถบเมนู' : 'ย่อแถบเมนู'}
			aria-expanded={!collapsed}
			onclick={() => (collapsed = !collapsed)}
			>{#if collapsed}<PanelLeftOpen class="size-4" />{:else}<PanelLeftClose
					class="size-4"
				/>{/if}</Button
		>
	</aside>

	<div class="workspace-main">
		<header class="topbar">
			<Sheet.Root bind:open={mobileOpen}>
				<Sheet.Trigger
					class={`${buttonVariants({ variant: 'ghost', size: 'icon' })} mobile-menu`}
					aria-label="เปิดเมนู"><Menu class="size-5" /></Sheet.Trigger
				>
				<Sheet.Content
					side="left"
					portalProps={{ disabled: true }}
					class="preview-sheet w-[min(320px,88vw)] gap-0 overflow-y-auto p-0"
				>
					<Sheet.Header class="sr-only"
						><Sheet.Title>เมนูหลักตัวอย่าง</Sheet.Title><Sheet.Description
							>พื้นที่ทำงานและบริการของโรงเรียน</Sheet.Description
						></Sheet.Header
					>
					{@render navigation(false)}
				</Sheet.Content>
			</Sheet.Root>
			<div class="topbar-location">
				<span class="location-mark"><Home class="size-3.5" /></span><span>พื้นที่ทำงาน</span
				><ChevronLeft class="size-3 rotate-180" /><strong>หน้าหลัก</strong>
			</div>
			<div class="header-actions">
				<div class="academic-selectors">
					<CalendarDays class="context-icon size-4 text-muted-foreground" />
					<Select.Root type="single" bind:value={year}>
						<Select.Trigger aria-label="ปีการศึกษา" class="year-select"
							>ปีการศึกษา {year}</Select.Trigger
						>
						<Select.Content portalProps={{ disabled: true }}
							>{#each ['2569', '2568'] as value (value)}<Select.Item {value}
									>ปีการศึกษา {value}</Select.Item
								>{/each}</Select.Content
						>
					</Select.Root>
					<Select.Root type="single" bind:value={term}>
						<Select.Trigger aria-label="ภาคเรียน" class="term-select"
							>ภาคเรียน {term}</Select.Trigger
						>
						<Select.Content portalProps={{ disabled: true }}
							>{#each ['1', '2'] as value (value)}<Select.Item {value}>ภาคเรียน {value}</Select.Item
								>{/each}</Select.Content
						>
					</Select.Root>
				</div>
				<Button
					variant="ghost"
					size="icon"
					aria-label={dark ? 'ใช้ธีมสว่าง' : 'ใช้ธีมมืด'}
					onclick={() => (dark = !dark)}
					>{#if dark}<Sun class="size-4" />{:else}<Moon class="size-4" />{/if}</Button
				>
				<Button
					variant="ghost"
					size="icon"
					class="notification-button"
					aria-label="การแจ้งเตือนตัวอย่าง"
					onclick={() => showDemo('การแจ้งเตือน')}
					><Bell class="size-4" /><span class="notification-dot"></span></Button
				>
				<Button
					variant="ghost"
					class="profile-button"
					aria-label="บัญชีครูพิมพ์ชนก ใจดี ตัวอย่าง"
					onclick={() => showDemo('บัญชีผู้ใช้งาน')}
					><span class="avatar">พ</span><span class="profile-name"
						>พิมพ์ชนก<ChevronDown class="size-3" /></span
					></Button
				>
			</div>
		</header>
		<main>
			<PreviewShell
				contentClass={compactDensity ? 'space-y-4' : 'space-y-6'}
				title="หน้าหลักของฉัน"
				description="งานที่ต้องติดตามและบริการของโรงเรียนที่คุณใช้งานได้"
			>
				{#snippet meta()}<p class="preview-label">
						<span></span>ตัวอย่างดีไซน์ · ข้อมูลสมมติ
					</p>{/snippet}
				{#snippet actions()}<Badge variant="outline" class="welcome-date"
						><CalendarDays class="size-3.5" />จันทร์ 18 พฤษภาคม 2569</Badge
					>{/snippet}

				<Card class="welcome-card gap-0 overflow-hidden py-0">
					<CardContent class="welcome-content">
						<div class="welcome-person">
							<div class="welcome-avatar">พ<span><Check class="size-3" /></span></div>
							<div>
								<p class="welcome-eyebrow">ยินดีต้อนรับกลับ</p>
								<h2>ครูพิมพ์ชนก ใจดี</h2>
								<p class="welcome-role">ครู · กลุ่มสาระการเรียนรู้ภาษาไทย</p>
							</div>
						</div>
						<Button class="gap-2" onclick={() => showDemo('งานของฉัน')}
							><Inbox class="size-4" />เปิดงานของฉัน<ArrowRight class="size-4" /></Button
						>
					</CardContent>
				</Card>

				<section aria-labelledby="personal-summary-title" class="space-y-3">
					<div class="section-heading">
						<div>
							<h2 id="personal-summary-title">สรุปของฉัน</h2>
							<p>รายการที่ควรทราบก่อนเริ่มงานวันนี้</p>
						</div>
						<span class="section-kicker">MY WORKSPACE</span>
					</div>
					<div class="summary-grid">
						{#each summaries as summary (summary.label)}<Card class="summary-card gap-0 py-0"
								><CardContent class="summary-content"
									><div>
										<p class="summary-label">{summary.label}</p>
										<p class:overdue={summary.tone === 'destructive'} class="summary-number">
											{summary.value}
										</p>
										<p class="summary-note">{summary.note}</p>
									</div>
									<span class={`summary-icon ${summary.tone}`}><summary.icon class="size-5" /></span
									></CardContent
								></Card
							>{/each}
					</div>
				</section>

				<section aria-labelledby="services-title" class="space-y-3">
					<div class="section-heading">
						<div>
							<h2 id="services-title">บริการของโรงเรียน</h2>
							<p>เลือกกลุ่มบริหารและฝ่าย/งาน เหมือนไปติดต่อหน่วยงานภายในโรงเรียน</p>
						</div>
						<span class="section-kicker">SCHOOL SERVICES</span>
					</div>
					<div class="service-columns">
						{#each workspaces as workspace (workspace.name)}<Card class="service-card gap-0 py-0"
								><CardHeader class="service-header"
									><span class="workspace-icon"><workspace.icon class="size-5" /></span>
									<div>
										<CardTitle class="text-base">{workspace.name}</CardTitle><CardDescription
											class="mt-1 text-sm"
											>{workspace.sections.reduce(
												(count, section) => count + section.items.length,
												0
											)} บริการ</CardDescription
										>
									</div></CardHeader
								><CardContent class="service-content"
									>{#each workspace.sections as section (section.name)}<div class="space-y-2">
											<h3 class="service-section">
												<section.icon class="size-3.5" />{section.name}
											</h3>
											<div class="service-links">
												{#each section.items as item (item.name)}<Button
														variant="outline"
														class="service-link"
														onclick={() => showDemo(item.name)}
														><item.icon class="size-4 shrink-0 text-primary" /><span
															>{item.name}</span
														><ArrowRight class="service-arrow size-3.5 shrink-0" /></Button
													>{/each}
											</div>
										</div>{/each}</CardContent
								></Card
							>{/each}
					</div>
				</section>

				<section aria-labelledby="school-summary-title" class="space-y-3">
					<div class="section-heading">
						<div>
							<h2 id="school-summary-title">ภาพรวมโรงเรียน</h2>
							<p>ข้อมูลรวมสำหรับประกอบการทำงาน</p>
						</div>
						<Badge variant="secondary" class="text-sm">ปีการศึกษา {year} · ภาคเรียน {term}</Badge>
					</div>
					<div class="overview-grid">
						{#each overview as item (item.label)}<Card class="overview-card gap-0 py-0"
								><CardContent class="overview-content"
									><span class="overview-icon"><item.icon class="size-5" /></span>
									<div>
										<p>{item.label}</p>
										<strong>{item.value}<span>{item.unit}</span></strong>
									</div></CardContent
								></Card
							>{/each}
					</div>
				</section>
			</PreviewShell>
		</main>
	</div>

	<Dialog.Root bind:open={demoOpen}>
		<Dialog.Content portalProps={{ disabled: true }}>
			<Dialog.Header
				><Badge variant="secondary" class="mb-2">ตัวอย่างดีไซน์</Badge><Dialog.Title
					>{demoTitle}</Dialog.Title
				><Dialog.Description
					>นี่คือการสาธิตหน้าตาและการโต้ตอบของ SchoolOrbit ข้อมูลทั้งหมดเป็นข้อมูลสมมติ
					และไม่มีการบันทึกหรือเชื่อมต่อบริการจริง</Dialog.Description
				></Dialog.Header
			>
			<Dialog.Footer
				><Button onclick={() => (demoOpen = false)}
					>กลับสู่หน้าตัวอย่าง<ArrowRight class="size-4" /></Button
				></Dialog.Footer
			>
		</Dialog.Content>
	</Dialog.Root>
</div>

<style>
	.design-preview {
		--background: #f6f7f3;
		--foreground: #203a37;
		--card: #ffffff;
		--card-foreground: #203a37;
		--popover: #ffffff;
		--popover-foreground: #203a37;
		--primary: #167166;
		--primary-foreground: #ffffff;
		--secondary: #eaf2ed;
		--secondary-foreground: #326459;
		--muted: #edf1ed;
		--muted-foreground: #61746d;
		--accent: #eaf3ee;
		--accent-foreground: #145e55;
		--destructive: #b54441;
		--border: #e0e7df;
		--input: #d8e2d9;
		--ring: #238276;
		--radius: 0.8rem;
		--warning: #98651b;
		--warning-surface: #fcf3df;
		--destructive-surface: #fbecea;
		--welcome: #e8f1e9;
		--sidebar: #fcfdf9;
		font-family: var(--preview-font, 'School Kanit'), sans-serif;
		color: var(--foreground);
		background: var(--background);
		min-height: 100vh;
		display: flex;
		line-height: 1.5;
		color-scheme: light;
	}
	.design-preview.dark {
		--background: #152521;
		--foreground: #e8efe8;
		--card: #1d302a;
		--card-foreground: #e8efe8;
		--popover: #243b33;
		--popover-foreground: #e8efe8;
		--primary: #81cdb6;
		--primary-foreground: #14382c;
		--secondary: #2a4438;
		--secondary-foreground: #bcdfca;
		--muted: #263b32;
		--muted-foreground: #afc1b4;
		--accent: #2e4b3e;
		--accent-foreground: #d3eadd;
		--destructive: #f4a49a;
		--border: #354b3e;
		--input: #415b4b;
		--ring: #8bd3ba;
		--warning: #efc37c;
		--warning-surface: #453d2c;
		--destructive-surface: #4a3531;
		--welcome: #294535;
		--sidebar: #192d25;
		color-scheme: dark;
	}
	.design-preview :global(button),
	.design-preview :global([role='dialog']),
	.design-preview :global([role='listbox']) {
		font-family: inherit;
	}
	.design-preview :global([data-slot='badge']) {
		font-size: 12px;
	}
	.design-preview :global([data-slot='card']) {
		box-shadow: 0 2px 3px color-mix(in srgb, var(--foreground) 2%, transparent);
	}
	.desktop-sidebar {
		width: 254px;
		flex-shrink: 0;
		min-height: 100vh;
		background: var(--sidebar);
		border-right: 1px solid var(--border);
		position: relative;
		display: flex;
		flex-direction: column;
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 79px;
		padding: 0 22px;
	}
	.brand-mark {
		display: grid;
		place-items: center;
		width: 36px;
		height: 36px;
		background: var(--primary);
		color: var(--primary-foreground);
		border-radius: 11px;
		flex-shrink: 0;
	}
	.brand strong {
		font-size: 20px;
		letter-spacing: -0.8px;
	}
	.brand-dot {
		color: var(--primary);
	}
	.brand p {
		color: var(--muted-foreground);
		font-size: 12px;
		margin-top: 1px;
	}
	.school-identity {
		display: flex;
		align-items: center;
		gap: 10px;
		margin: 0 16px 22px;
		padding: 12px;
		border: 1px solid var(--border);
		border-radius: 10px;
		background: var(--card);
	}
	.school-mark {
		color: var(--primary);
	}
	.school-identity strong {
		font-size: 12px;
	}
	.school-identity p {
		font-size: 12px;
		color: var(--muted-foreground);
		margin-top: 3px;
	}
	.navigation {
		padding: 0 14px 20px;
	}
	.nav-eyebrow {
		font-size: 12px;
		color: var(--muted-foreground);
		padding: 0 10px 9px;
	}
	.design-preview :global(.nav-link) {
		display: flex;
		width: 100%;
		justify-content: flex-start;
		gap: 10px;
		height: 36px;
		font-size: 12px;
		font-weight: 400;
		padding: 0 11px;
		color: var(--muted-foreground);
	}
	.design-preview :global(.nav-link.active) {
		background: var(--primary);
		color: var(--primary-foreground);
		font-weight: 600;
		box-shadow: 0 4px 10px color-mix(in srgb, var(--primary) 12%, transparent);
		margin-bottom: 4px;
	}
	.active-dot {
		margin-left: auto;
		height: 5px;
		width: 5px;
		border-radius: 50%;
		background: currentColor;
	}
	.nav-workspace {
		margin-top: 22px;
	}
	.workspace-name {
		display: flex;
		align-items: center;
		gap: 7px;
		padding: 0 10px 6px;
		color: var(--foreground);
		font-size: 12px;
		font-weight: 600;
	}
	.design-preview :global(.section-toggle) {
		width: 100%;
		display: flex;
		justify-content: space-between;
		font-size: 12px;
		color: var(--muted-foreground);
		height: 30px;
		padding: 0 10px;
	}
	.nav-items {
		margin-left: 14px;
		padding-left: 3px;
		border-left: 1px solid var(--border);
	}
	.design-preview :global(.nav-items .nav-link) {
		font-size: 12px;
		height: 32px;
		gap: 8px;
	}
	.sidebar-footer {
		margin-top: auto;
		padding: 20px 24px;
		border-top: 1px solid var(--border);
	}
	.sidebar-footer p {
		font-size: 12px;
		color: var(--muted-foreground);
		margin-top: 4px;
	}
	.connection {
		font-size: 12px;
		color: var(--muted-foreground);
		display: flex;
		align-items: center;
		gap: 7px;
	}
	.connection span {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background: var(--primary);
	}
	.design-preview :global(.collapse-control) {
		position: absolute;
		right: -14px;
		top: 25px;
		z-index: 40;
		width: 27px;
		height: 27px;
		border-radius: 50%;
		background: var(--card);
	}
	.collapsed .desktop-sidebar {
		width: 76px;
	}
	.collapsed .brand {
		padding: 0 20px;
	}
	.collapsed .navigation {
		padding: 0 18px 20px;
	}
	.collapsed :global(.nav-link) {
		padding: 0;
		justify-content: center;
	}
	.collapsed .nav-workspace {
		display: flex;
		justify-content: center;
	}
	.collapsed .sidebar-footer {
		padding: 24px 27px;
	}
	.workspace-main {
		flex: 1;
		min-width: 0;
	}
	.topbar {
		min-height: 65px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 12px 24px;
		border-bottom: 1px solid var(--border);
		background: var(--card);
	}
	.topbar-location {
		display: flex;
		gap: 10px;
		align-items: center;
		font-size: 12px;
		color: var(--muted-foreground);
	}
	.topbar-location strong {
		font-weight: 400;
		color: var(--foreground);
	}
	.location-mark {
		display: grid;
		place-items: center;
		width: 27px;
		height: 27px;
		border-radius: 7px;
		background: var(--muted);
	}
	.header-actions {
		display: flex;
		align-items: center;
		gap: 5px;
		margin-left: auto;
	}
	.academic-selectors {
		display: flex;
		align-items: center;
		gap: 8px;
		padding-right: 12px;
		margin-right: 7px;
		border-right: 1px solid var(--border);
	}
	.design-preview :global(.year-select),
	.design-preview :global(.term-select) {
		font-size: 12px;
		border-color: transparent;
		background: var(--background);
		box-shadow: none;
		padding: 0 9px;
	}
	.design-preview :global(.notification-button) {
		position: relative;
	}
	.notification-dot {
		position: absolute;
		right: 10px;
		top: 9px;
		width: 4px;
		height: 4px;
		border-radius: 50%;
		background: var(--primary);
	}
	.design-preview :global(.profile-button) {
		gap: 8px;
		height: 36px;
		padding: 0 0 0 10px;
		margin-left: 5px;
		border-left: 1px solid var(--border);
		border-radius: 0;
	}
	.avatar {
		display: grid;
		place-items: center;
		height: 30px;
		width: 30px;
		border-radius: 50%;
		background: var(--welcome);
		color: var(--primary);
		font-size: 14px;
		font-weight: 600;
	}
	.profile-name {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 12px;
	}
	.design-preview :global(.mobile-menu) {
		display: none;
	}
	.preview-label {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.preview-label span {
		height: 5px;
		width: 5px;
		background: var(--primary);
		border-radius: 50%;
	}
	.design-preview :global(.welcome-date) {
		background: var(--card);
		color: var(--muted-foreground);
		font-size: 12px;
		font-weight: 400;
		padding: 7px 10px;
		margin-top: 15px;
	}
	.design-preview :global(.welcome-card) {
		border-color: color-mix(in srgb, var(--primary) 18%, var(--border));
		background: linear-gradient(110deg, var(--welcome), var(--card));
	}
	.design-preview :global(.welcome-content) {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 20px;
		padding: 24px;
	}
	.welcome-person {
		display: flex;
		align-items: center;
		gap: 16px;
	}
	.welcome-avatar {
		position: relative;
		display: grid;
		place-items: center;
		height: 58px;
		width: 58px;
		border: 3px solid var(--card);
		border-radius: 50%;
		color: var(--primary);
		background: color-mix(in srgb, var(--primary) 12%, var(--card));
		font-size: 24px;
		font-weight: 600;
		flex-shrink: 0;
	}
	.welcome-avatar span {
		position: absolute;
		right: -3px;
		bottom: -2px;
		background: var(--primary);
		color: var(--primary-foreground);
		border: 2px solid var(--card);
		border-radius: 50%;
		padding: 2px;
	}
	.welcome-eyebrow {
		font-size: 12px;
		color: var(--primary);
	}
	.welcome-person h2 {
		font-size: 23px;
		font-weight: 600;
		letter-spacing: -0.4px;
		margin: 2px 0;
	}
	.welcome-role {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.section-heading {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
	}
	.section-heading h2 {
		font-size: 16px;
		font-weight: 600;
	}
	.section-heading p {
		font-size: 12px;
		color: var(--muted-foreground);
		margin-top: 2px;
	}
	.section-kicker {
		font-size: 12px;
		letter-spacing: 1.5px;
		color: var(--muted-foreground);
	}
	.summary-grid {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 12px;
	}
	.design-preview :global(.summary-content) {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 8px;
		padding: 17px;
	}
	.summary-label {
		font-size: 12px;
		color: var(--muted-foreground);
	}
	.summary-number {
		font-size: 29px;
		font-weight: 600;
		line-height: 1.5;
		letter-spacing: -0.7px;
	}
	.summary-number.overdue {
		color: var(--destructive);
	}
	.summary-note {
		font-size: 12px;
		color: var(--muted-foreground);
	}
	.summary-icon {
		display: grid;
		place-items: center;
		height: 36px;
		width: 36px;
		border-radius: 10px;
		flex-shrink: 0;
	}
	.summary-icon.primary {
		background: var(--secondary);
		color: var(--primary);
	}
	.summary-icon.warning {
		background: var(--warning-surface);
		color: var(--warning);
	}
	.summary-icon.destructive {
		background: var(--destructive-surface);
		color: var(--destructive);
	}
	.service-columns {
		columns: 2;
		gap: 14px;
	}
	.design-preview :global(.service-card) {
		break-inside: avoid;
		margin-bottom: 14px;
	}
	.design-preview :global(.service-card:last-child) {
		margin-bottom: 0;
	}
	.design-preview :global(.service-header) {
		display: flex;
		align-items: center;
		gap: 11px;
		padding: 16px 18px;
		border-bottom: 1px solid var(--border);
	}
	.workspace-icon {
		display: grid;
		place-items: center;
		width: 37px;
		height: 37px;
		border-radius: 10px;
		background: var(--secondary);
		color: var(--primary);
		flex-shrink: 0;
	}
	.design-preview :global(.service-content) {
		display: grid;
		gap: 16px;
		padding: 16px 18px 18px;
	}
	.service-section {
		display: flex;
		align-items: center;
		gap: 7px;
		font-size: 12px;
		color: var(--muted-foreground);
		font-weight: 600;
	}
	.service-links {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 8px;
	}
	.design-preview :global(.service-link) {
		width: 100%;
		height: auto;
		min-height: 42px;
		justify-content: flex-start;
		gap: 8px;
		font-size: 12px;
		font-weight: 400;
		white-space: normal;
		text-align: left;
		padding: 10px;
		box-shadow: none;
		background: var(--background);
	}
	.service-arrow {
		margin-left: auto;
		color: var(--muted-foreground);
	}
	.overview-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 12px;
	}
	.design-preview :global(.overview-content) {
		display: flex;
		gap: 14px;
		align-items: center;
		padding: 18px;
	}
	.overview-icon {
		display: grid;
		place-items: center;
		color: var(--primary);
		background: var(--secondary);
		height: 42px;
		width: 42px;
		border-radius: 10px;
		flex-shrink: 0;
	}
	:global(.overview-content) p {
		font-size: 12px;
		color: var(--muted-foreground);
	}
	:global(.overview-content) strong {
		font-size: 23px;
		font-weight: 600;
		line-height: 1.5;
	}
	:global(.overview-content) strong span {
		font-size: 12px;
		font-weight: 400;
		color: var(--muted-foreground);
		margin-left: 7px;
	}
	.design-preview :global(.preview-sheet) {
		background: var(--sidebar);
	}
	.design-preview :global(.preview-sheet .brand) {
		padding-right: 40px;
	}
	@container (min-width: 1600px) {
		.desktop-sidebar {
			width: 270px;
		}
	}
	@container (max-width: 1199px) {
		.topbar-location {
			display: none;
		}
		.summary-grid {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
		.service-columns {
			columns: 1;
		}
		.section-kicker {
			display: none;
		}
	}
	@container (max-width: 1023px) {
		.desktop-sidebar {
			display: none;
		}
		.design-preview :global(.mobile-menu) {
			display: grid;
			place-items: center;
			height: 36px;
			width: 36px;
			border-radius: 8px;
			color: var(--foreground);
			flex-shrink: 0;
		}
		.design-preview :global(.mobile-menu:focus-visible) {
			outline: 2px solid var(--ring);
			outline-offset: 2px;
		}
		.topbar {
			padding: 12px 16px;
		}
	}
	@container (max-width: 639px) {
		.topbar {
			gap: 4px;
			padding: 10px 12px;
			min-height: 62px;
		}
		.header-actions {
			gap: 0;
		}
		.academic-selectors {
			gap: 3px;
			padding-right: 2px;
			margin-right: 0;
			border: 0;
		}
		.context-icon,
		.profile-name {
			display: none;
		}
		.design-preview :global(.year-select),
		.design-preview :global(.term-select) {
			font-size: 12px;
			gap: 3px;
			padding: 0 6px;
		}
		.design-preview :global(.profile-button) {
			padding-left: 4px;
			margin-left: 0;
			border: 0;
		}
		.design-preview :global(.notification-button) {
			display: none;
		}
		.design-preview :global(.welcome-date) {
			margin-top: 0;
		}
		.design-preview :global(.welcome-content) {
			padding: 20px;
			align-items: flex-start;
			flex-direction: column;
		}
		.welcome-person {
			gap: 12px;
		}
		.welcome-person h2 {
			font-size: 21px;
		}
		.welcome-avatar {
			height: 48px;
			width: 48px;
			font-size: 21px;
		}
		.summary-grid {
			gap: 10px;
		}
		.design-preview :global(.summary-content) {
			padding: 14px;
			flex-direction: column-reverse;
			gap: 8px;
		}
		.summary-note {
			font-size: 12px;
		}
		.service-links {
			grid-template-columns: 1fr;
		}
		.overview-grid {
			grid-template-columns: 1fr;
		}
		.section-heading {
			align-items: flex-start;
		}
		.section-heading :global([data-slot='badge']) {
			max-width: 130px;
			white-space: normal;
			text-align: center;
		}
	}
	@container (max-width: 359px) {
		.header-actions {
			flex: 1;
			min-width: 0;
			flex-wrap: wrap;
			justify-content: flex-end;
		}
		.academic-selectors {
			width: 100%;
			justify-content: flex-end;
		}
		.topbar {
			align-items: flex-start;
		}
	}
	.compact-density :global(.welcome-content) {
		padding: 18px;
	}
	.compact-density :global(.summary-content) {
		padding: 14px;
	}
	.compact-density :global(.service-header) {
		padding: 12px 16px;
	}
	.compact-density :global(.service-content) {
		padding: 12px 16px;
		gap: 12px;
	}
	.compact-density :global(.overview-content) {
		padding: 14px;
	}
	.compact-density .nav-workspace {
		margin-top: 16px;
	}
</style>
