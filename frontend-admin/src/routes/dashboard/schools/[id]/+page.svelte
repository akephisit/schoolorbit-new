<script lang="ts">
	import { page } from '$app/stores';
	import { apiClient, type School } from '$lib/api/client';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	
	let schoolId = $derived($page.params.id);
	let school = $state<School | null>(null);
	let loading = $state(true);
	let error = $state('');
	
	onMount(async () => {
		await loadSchool();
	});
	
	async function loadSchool() {
		loading = true;
		error = '';
		
		try {
			const response = await apiClient.getSchool(schoolId);
			if (response.success && response.data) {
				school = response.data;
			} else {
				error = response.error || 'Failed to load school';
			}
		} catch (e: any) {
			error = e.message || 'Failed to load school';
		} finally {
			loading = false;
		}
	}
	
	function formatDate(dateString: string) {
		return new Date(dateString).toLocaleString('th-TH', {
			dateStyle: 'medium',
			timeStyle: 'short'
		});
	}
	
	function getStatusColor(status: string) {
		switch (status) {
			case 'active': return 'bg-green-100 text-green-800';
			case 'provisioning': return 'bg-blue-100 text-blue-800';
			case 'deployment_failed': return 'bg-red-100 text-red-800';
			default: return 'bg-gray-100 text-gray-800';
		}
	}
	
	function getStatusText(status: string) {
		switch (status) {
			case 'active': return '✅ พร้อมใช้งาน';
			case 'provisioning': return '⏳ กำลังสร้าง';
			case 'deployment_failed': return '❌ ล้มเหลว';
			default: return status;
		}
	}
</script>

{#if loading}
	<div class="loading-container">
		<div class="spinner"></div>
		<p>กำลังโหลดข้อมูล...</p>
	</div>
{:else if error}
	<div class="error-container">
		<h2>เกิดข้อผิดพลาด</h2>
		<p>{error}</p>
		<button onclick={() => goto('/dashboard/schools')} class="btn-primary">
			กลับไปหน้ารายการโรงเรียน
		</button>
	</div>
{:else if school}
	<div class="school-detail">
		<div class="header">
			<button onclick={() => goto('/dashboard/schools')} class="back-button"> ← กลับ </button>
			<h1>{school.name}</h1>
			<span class="status-badge {getStatusColor(school.status)}">
				{getStatusText(school.status)}
			</span>
		</div>

		<div class="detail-grid">
			<!-- Basic Info -->
			<div class="card">
				<h2>ข้อมูลทั่วไป</h2>
				<div class="info-row">
					<span class="label">ID:</span>
					<span class="value font-mono">{school.id}</span>
				</div>
				<div class="info-row">
					<span class="label">ชื่อ:</span>
					<span class="value">{school.name}</span>
				</div>
				<div class="info-row">
					<span class="label">Subdomain:</span>
					<span class="value">{school.subdomain}</span>
				</div>
				<div class="info-row">
					<span class="label">Database:</span>
					<span class="value font-mono">{school.dbName}</span>
				</div>
			</div>

			<!-- URLs -->
			<div class="card">
				<h2>URLs</h2>
				{#if school.subdomainUrl || (school.config && school.config.deployment_url)}
					<div class="info-row">
						<span class="label">Frontend URL:</span>
						<a
							href={school.subdomainUrl || school.config.deployment_url}
							target="_blank"
							rel="noopener noreferrer"
							class="link"
						>
							{school.subdomainUrl || school.config.deployment_url} ↗
						</a>
					</div>
				{:else}
					<p class="text-gray-500">ยังไม่มี deployment URL</p>
				{/if}
			</div>

			<!-- Configuration -->
			<div class="card">
				<h2>Configuration</h2>
				{#if school.config}
					<pre class="config-json">{JSON.stringify(school.config, null, 2)}</pre>
				{:else}
					<p class="text-gray-500">ไม่มีข้อมูล configuration</p>
				{/if}
			</div>

			<!-- Timestamps -->
			<div class="card">
				<h2>Timeline</h2>
				<div class="info-row">
					<span class="label">สร้างเมื่อ:</span>
					<span class="value">{formatDate(school.createdAt)}</span>
				</div>
				<div class="info-row">
					<span class="label">แก้ไขล่าสุด:</span>
					<span class="value">{formatDate(school.updatedAt)}</span>
				</div>
			</div>
		</div>

		<!-- Actions -->
		<div class="actions">
			<button onclick={() => goto(`/dashboard/schools`)} class="btn-secondary">
				กลับไปหน้ารายการ
			</button>
			{#if school.status === 'deployment_failed' && school.config && school.config.error}
				<div class="error-message">
					<strong>Error:</strong>
					{school.config.error}
				</div>
			{/if}
		</div>
	</div>
{/if}

<style>
	.loading-container {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 400px;
		gap: 1rem;
	}

	.spinner {
		width: 48px;
		height: 48px;
		border: 4px solid #e2e8f0;
		border-top-color: #667eea;
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to { transform: rotate(360deg); }
	}

	.error-container {
		text-align: center;
		padding: 3rem;
	}

	.error-container h2 {
		color: #e53e3e;
		margin-bottom: 1rem;
	}

	.school-detail {
		max-width: 1200px;
		margin: 0 auto;
		padding: 2rem;
	}

	.header {
		display: flex;
		align-items: center;
		gap: 1rem;
		margin-bottom: 2rem;
		flex-wrap: wrap;
	}

	.back-button {
		padding: 0.5rem 1rem;
		background: white;
		border: 2px solid #e2e8f0;
		border-radius: 8px;
		cursor: pointer;
		font-weight: 600;
		transition: all 0.2s;
	}

	.back-button:hover {
		border-color: #667eea;
		color: #667eea;
	}

	.header h1 {
		flex: 1;
		font-size: 2rem;
		font-weight: 700;
		color: #1a202c;
		margin: 0;
	}

	.status-badge {
		padding: 0.5rem 1rem;
		border-radius: 12px;
		font-size: 0.875rem;
		font-weight: 600;
	}

	.detail-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
		gap: 1.5rem;
		margin-bottom: 2rem;
	}

	.card {
		background: white;
		padding: 1.5rem;
		border-radius: 12px;
		box-shadow: 0 4px 16px rgba(0, 0, 0, 0.1);
	}

	.card h2 {
		font-size: 1.25rem;
		font-weight: 600;
		color: #2d3748;
		margin-bottom: 1rem;
	}

	.info-row {
		display: grid;
		grid-template-columns: 120px 1fr;
		gap: 1rem;
		padding: 0.75rem 0;
		border-bottom: 1px solid #e2e8f0;
	}

	.info-row:last-child {
		border-bottom: none;
	}

	.label {
		font-weight: 600;
		color: #4a5568;
	}

	.value {
		color: #2d3748;
	}

	.font-mono {
		font-family: monospace;
		font-size: 0.875rem;
	}

	.link {
		color: #667eea;
		text-decoration: none;
		font-weight: 600;
		transition: color 0.2s;
	}

	.link:hover {
		color: #764ba2;
		text-decoration: underline;
	}

	.config-json {
		background: #f7fafc;
		padding: 1rem;
		border-radius: 8px;
		font-family: monospace;
		font-size: 0.875rem;
		overflow-x: auto;
		color: #2d3748;
	}

	.text-gray-500 {
		color: #a0aec0;
	}

	.actions {
		display: flex;
		gap: 1rem;
		align-items: center;
	}

	.btn-primary, .btn-secondary {
		padding: 0.75rem 1.5rem;
		border-radius: 8px;
		font-weight: 600;
		cursor: pointer;
		transition: all 0.2s;
		border: none;
	}

	.btn-primary {
		background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
		color: white;
	}

	.btn-primary:hover {
		transform: translateY(-2px);
		box-shadow: 0 8px 16px rgba(102, 126, 234, 0.3);
	}

	.btn-secondary {
		background: white;
		color: #667eea;
		border: 2px solid #667eea;
	}

	.btn-secondary:hover {
		background: #667eea;
		color: white;
	}

	.error-message {
		flex: 1;
		padding: 1rem;
		background: #fed7d7;
		color: #c53030;
		border-radius: 8px;
		border-left: 4px solid #e53e3e;
	}
</style>
