<script lang="ts">
	import { onMount } from 'svelte';
	import { migrationAPI, type MigrationStatusResponse } from '$lib/api/migration';
	import { toast } from 'svelte-sonner';

	let status: MigrationStatusResponse | null = null;
	let loading = false;
	let migrating = false;

	onMount(async () => {
		await loadStatus();
	});

	async function loadStatus() {
		loading = true;
		try {
			status = await migrationAPI.getStatus();
		} catch (error) {
			toast.error('Failed to load migration status');
			console.error(error);
		} finally {
			loading = false;
		}
	}

	async function migrateAll() {
		if (!confirm('Are you sure you want to migrate all schools?')) {
			return;
		}

		migrating = true;
		toast.info('Starting migration for all schools...');

		try {
			const result = await migrationAPI.migrateAll();
			toast.success(
				`Migration complete! ${result.success}/${result.total} schools migrated successfully`
			);
			await loadStatus();
		} catch (error) {
			toast.error('Migration failed');
			console.error(error);
		} finally {
			migrating = false;
		}
	}

	function getStatusBadgeClass(status: string): string {
		switch (status) {
			case 'migrated':
				return 'badge-success';
			case 'failed':
				return 'badge-error';
			case 'outdated':
				return 'badge-warning';
			default:
				return 'badge-neutral';
		}
	}

	function getStatusIcon(status: string): string {
		switch (status) {
			case 'migrated':
				return '✅';
			case 'failed':
				return '❌';
			case 'outdated':
				return '⚠️';
			default:
				return '⏳';
		}
	}

	function formatDate(dateStr?: string): string {
		if (!dateStr) return 'Never';
		const date = new Date(dateStr);
		return date.toLocaleString();
	}
</script>

<div class="migration-dashboard">
	<header>
		<h1>Database Migrations</h1>
		<button class="btn-primary" on:click={migrateAll} disabled={migrating || loading}>
			{#if migrating}
				<span class="spinner"></span>
				Migrating...
			{:else}
				🔄 Migrate All Schools
			{/if}
		</button>
	</header>

	{#if loading}
		<div class="loading">
			<div class="spinner large"></div>
			<p>Loading migration status...</p>
		</div>
	{:else if status}
		<div class="stats-grid">
			<div class="stat-card">
				<h3>Total Schools</h3>
				<p class="stat-value">{status.total_schools}</p>
			</div>
			<div class="stat-card success">
				<h3>Up to Date</h3>
				<p class="stat-value">{status.migrated}</p>
			</div>
			<div class="stat-card warning">
				<h3>Outdated</h3>
				<p class="stat-value">{status.outdated}</p>
			</div>
			<div class="stat-card pending">
				<h3>Pending</h3>
				<p class="stat-value">{status.pending}</p>
			</div>
			<div class="stat-card error">
				<h3>Failed</h3>
				<p class="stat-value">{status.failed}</p>
			</div>
			<div class="stat-card">
				<h3>Latest Version</h3>
				<p class="stat-value">v{status.latest_version}</p>
			</div>
		</div>

		<div class="table-container">
			<table>
				<thead>
					<tr>
						<th>School</th>
						<th>Status</th>
						<th>Version</th>
						<th>Last Migrated</th>
						<th>Error</th>
					</tr>
				</thead>
				<tbody>
					{#each status.schools as school}
						<tr>
							<td class="school-name">{school.subdomain}</td>
							<td>
								<span class="badge {getStatusBadgeClass(school.migration_status)}">
									{getStatusIcon(school.migration_status)}
									{school.migration_status}
								</span>
							</td>
							<td>
								v{school.migration_version}
								{#if school.migration_version < status.latest_version}
									<span class="outdated-tag">→ v{status.latest_version}</span>
								{/if}
							</td>
							<td class="date">{formatDate(school.last_migrated_at)}</td>
							<td class="error-cell">
								{#if school.migration_error}
									<details>
										<summary>View Error</summary>
										<pre>{school.migration_error}</pre>
									</details>
								{:else}
									-
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</div>

<style>
	.migration-dashboard {
		padding: 2rem;
		max-width: 1400px;
		margin: 0 auto;
	}

	header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 2rem;
	}

	h1 {
		font-size: 2rem;
		font-weight: 700;
		color: var(--color-text-primary, #111);
	}

	.btn-primary {
		padding: 0.75rem 1.5rem;
		background: var(--color-primary, #3b82f6);
		color: white;
		border: none;
		border-radius: 0.5rem;
		font-weight: 600;
		cursor: pointer;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		transition: all 0.2s;
	}

	.btn-primary:hover:not(:disabled) {
		background: var(--color-primary-dark, #2563eb);
		transform: translateY(-2px);
		box-shadow: 0 4px 12px rgba(59, 130, 246, 0.3);
	}

	.btn-primary:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.stats-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
		gap: 1rem;
		margin-bottom: 2rem;
	}

	.stat-card {
		background: white;
		padding: 1.5rem;
		border-radius: 0.75rem;
		border: 1px solid #e5e7eb;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
	}

	.stat-card h3 {
		font-size: 0.875rem;
		color: #6b7280;
		margin-bottom: 0.5rem;
	}

	.stat-value {
		font-size: 2rem;
		font-weight: 700;
		color: #111;
	}

	.stat-card.success .stat-value {
		color: #10b981;
	}

	.stat-card.warning .stat-value {
		color: #f59e0b;
	}

	.stat-card.error .stat-value {
		color: #ef4444;
	}

	.stat-card.pending .stat-value {
		color: #6366f1;
	}

	.table-container {
		background: white;
		border-radius: 0.75rem;
		border: 1px solid #e5e7eb;
		overflow: hidden;
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
	}

	table {
		width: 100%;
		border-collapse: collapse;
	}

	thead {
		background: #f9fafb;
		border-bottom: 1px solid #e5e7eb;
	}

	th {
		padding: 1rem;
		text-align: left;
		font-size: 0.875rem;
		font-weight: 600;
		color: #6b7280;
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	td {
		padding: 1rem;
		border-bottom: 1px solid #f3f4f6;
	}

	tbody tr:hover {
		background: #f9fafb;
	}

	.school-name {
		font-weight: 600;
		color: #111;
	}

	.badge {
		display: inline-flex;
		align-items: center;
		gap: 0.25rem;
		padding: 0.25rem 0.75rem;
		border-radius: 9999px;
		font-size: 0.875rem;
		font-weight: 500;
	}

	.badge-success {
		background: #d1fae5;
		color: #065f46;
	}

	.badge-error {
		background: #fee2e2;
		color: #991b1b;
	}

	.badge-warning {
		background: #fef3c7;
		color: #92400e;
	}

	.badge-neutral {
		background: #e5e7eb;
		color: #374151;
	}

	.outdated-tag {
		font-size: 0.75rem;
		color: #f59e0b;
		font-weight: 600;
	}

	.date {
		font-size: 0.875rem;
		color: #6b7280;
	}

	.error-cell details {
		cursor: pointer;
	}

	.error-cell summary {
		color: #ef4444;
		font-weight: 500;
	}

	.error-cell pre {
		margin-top: 0.5rem;
		padding: 0.5rem;
		background: #fee2e2;
		border-radius: 0.25rem;
		font-size: 0.75rem;
		overflow-x: auto;
	}

	.loading {
		text-align: center;
		padding: 4rem;
	}

	.spinner {
		display: inline-block;
		width: 1rem;
		height: 1rem;
		border: 2px solid rgba(255, 255, 255, 0.3);
		border-top-color: white;
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	.spinner.large {
		width: 3rem;
		height: 3rem;
		border-width: 3px;
		border-color: #e5e7eb;
		border-top-color: #3b82f6;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
