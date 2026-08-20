<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { Award, FilePenLine } from 'lucide-svelte';
	import { PERMISSION_MODULES, PERMISSIONS } from '$lib/permissions/registry';
	import { can } from '$lib/stores/permissions';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
	const permissions = $derived($can);
	const canViewIssued = $derived(permissions.has(PERMISSIONS.CERTIFICATE_READ_OWN));
	const canViewSelfRecorded = $derived(permissions.hasModule(PERMISSION_MODULES.ACHIEVEMENT));
</script>

<div class="achievement-workspace">
	<nav aria-label="ประเภทเกียรติบัตรและผลงาน">
		{#if canViewIssued}
			<a
				href={resolve('/staff/achievements/issued')}
				aria-current={page.url.pathname === '/staff/achievements/issued' ? 'page' : undefined}
			>
				<Award size={17} aria-hidden="true" /> ใบที่โรงเรียนออก
			</a>
		{/if}
		{#if canViewSelfRecorded}
			<a
				href={resolve('/staff/achievements/self-recorded')}
				aria-current={page.url.pathname === '/staff/achievements/self-recorded'
					? 'page'
					: undefined}
			>
				<FilePenLine size={17} aria-hidden="true" /> ผลงานที่บันทึกเอง
			</a>
		{/if}
	</nav>
	{@render children()}
</div>

<style>
	.achievement-workspace {
		min-width: 0;
	}

	nav {
		display: flex;
		gap: 0.35rem;
		width: fit-content;
		max-width: calc(100% - 2rem);
		margin: 1rem auto 0;
		padding: 0.3rem;
		border: 1px solid hsl(var(--border));
		background: hsl(var(--muted) / 0.48);
	}

	a {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		padding: 0.55rem 0.85rem;
		color: hsl(var(--muted-foreground));
		font-size: 0.86rem;
		font-weight: 600;
		text-decoration: none;
	}

	a[aria-current='page'] {
		background: hsl(var(--background));
		box-shadow: 0 2px 8px rgb(23 50 77 / 0.09);
		color: hsl(var(--foreground));
	}

	a:focus-visible {
		outline: 2px solid hsl(var(--ring));
		outline-offset: 2px;
	}

	@media (max-width: 520px) {
		nav {
			display: grid;
			width: calc(100% - 2rem);
		}

		a {
			justify-content: center;
		}
	}
</style>
