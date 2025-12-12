<script lang="ts">
	import { apiClient, type School, type CreateSchool } from '$lib/api/client';
	import { onMount } from 'svelte';
	
	let schools = $state<School[]>([]);
	let total = $state(0);
	let page = $state(1);
	let totalPages = $state(0);
	let loading = $state(true);
	let error = $state('');
	
	// Create school form
	let showCreateForm = $state(false);
	let createData = $state<CreateSchool>({
		name: '',
		subdomain: '',
		adminNationalId: '',
		adminPassword: ''
	});
	let creating = $state(false);
	
	onMount(async () => {
		await loadSchools();
	});
	
	async function loadSchools() {
		loading = true;
		error = '';
		
		try {
			const response = await apiClient.listSchools(page, 10);
			if (response.success && response.data) {
				schools = response.data.schools;
				total = response.data.total;
				totalPages = response.data.totalPages;
			} else {
				error = response.error || 'Failed to load schools';
			}
		} catch (e: any) {
			error = e.message || 'Failed to load schools';
		} finally {
			loading = false;
		}
	}
	
	async function handleCreateSchool(e: Event) {
		e.preventDefault();
		creating = true;
		error = '';
		
		try {
			const response = await apiClient.createSchool(createData);
			if (response.success) {
				showCreateForm = false;
				createData = {
					name: '',
					subdomain: '',
					adminNationalId: '',
					adminPassword: ''
				};
				await loadSchools();
			} else {
				error = response.error || 'Failed to create school';
			}
		} catch (e: any) {
			error = e.message || 'Failed to create school';
		} finally {
			creating = false;
		}
	}
	
	async function deleteSchool(id: string, name: string) {
		if (!confirm(`คุณแน่ใจหรือไม่ที่จะลบโรงเรียน "${name}"?`)) return;
		
		try {
			const response = await apiClient.deleteSchool(id);
			if (response.success) {
				await loadSchools();
			} else {
				error = response.error || 'Failed to delete school';
			}
		} catch (e: any) {
			error = e.message || 'Failed to delete school';
		}
	}
	
	// Deploy functionality
	let deploying = $state<string | null>(null);
	
	async function handleDeploy(schoolId: string, schoolName: string) {
		deploying = schoolId;
		error = '';
		
		try {
			const response = await apiClient.deploySchool(schoolId);
			if (response.success && response.data) {
				const githubUrl = response.data.githubActionsUrl || 'GitHub Actions';
				alert(`✅ Deployment triggered for ${schoolName}!\n\n${response.data.message}\n\nCheck progress: ${githubUrl}`);
				await loadSchools();
			} else {
				error = response.error || 'Deployment failed';
			}
		} catch (e: any) {
			error = e.message || 'Deployment failed';
		} finally {
			deploying = null;
		}
	}
	
	function nextPage() {
		if (page < totalPages) {
			page++;
			loadSchools();
		}
	}
	
	function prevPage() {
		if (page > 1) {
			page--;
			loadSchools();
		}
	}
</script>

<div class="schools-page">
	<div class="header">
		<h1>จัดการโรงเรียน</h1>
		<button class="btn-primary" onclick={() => (showCreateForm = !showCreateForm)}>
			{showCreateForm ? 'ยกเลิก' : '+ เพิ่มโรงเรียน'}
		</button>
	</div>

	{#if error}
		<div class="alert alert-error">{error}</div>
	{/if}

	{#if showCreateForm}
		<div class="create-form-card">
			<h2>สร้างโรงเรียนใหม่</h2>
			<form onsubmit={handleCreateSchool}>
				<div class="form-group">
					<label for="name">ชื่อโรงเรียน</label>
					<input
						type="text"
						id="name"
						bind:value={createData.name}
						required
						placeholder="โรงเรียนตัวอย่าง"
					/>
				</div>

				<div class="form-group">
					<label for="subdomain">Subdomain</label>
					<input
						type="text"
						id="subdomain"
						bind:value={createData.subdomain}
						required
						pattern="[a-z0-9-]+"
						placeholder="example-school"
					/>
					<small>ใช้ได้เฉพาะ a-z, 0-9, และ - เท่านั้น</small>
				</div>

				<div class="form-group">
					<label for="nationalId">เลขบัตรประชาชนผู้ดูแล</label>
					<input
						type="text"
						id="nationalId"
						bind:value={createData.adminNationalId}
						required
						pattern="[0-9]{13}"
						maxlength="13"
						placeholder="1234567890123"
					/>
					<small>13 หลัก</small>
				</div>

				<div class="form-group">
					<label for="password">รหัสผ่านผู้ดูแล</label>
					<input
						type="password"
						id="password"
						bind:value={createData.adminPassword}
						required
						minlength="6"
						placeholder="••••••••"
					/>
				</div>

				<button type="submit" class="btn-primary full-width" disabled={creating}>
					{creating ? 'กำลังสร้าง...' : 'สร้างโรงเรียน'}
				</button>
			</form>
		</div>
	{/if}

	{#if loading}
		<div class="loading">กำลังโหลด...</div>
	{:else if schools.length === 0}
		<div class="empty-state">
			<p>ยังไม่มีโรงเรียนในระบบ</p>
		</div>
	{:else}
		<div class="schools-grid">
			{#each schools as school}
				<div class="school-card">
					<div class="school-header">
						<h3>{school.name}</h3>
						<div class="status-badges">
							{#if school.status === 'provisioning'}
								<span class="status provisioning">
									<span class="spinner"></span>
									กำลังสร้าง
								</span>
							{:else if school.status === 'deployment_failed'}
								<span class="status failed"> ❌ ล้มเหลว </span>
							{:else if school.status === 'active'}
								<span class="status active"> ✅ พร้อมใช้งาน </span>
							{:else}
								<span class="status">
									{school.status}
								</span>
							{/if}
						</div>
					</div>

					<div class="school-info">
						<p><strong>Subdomain:</strong> {school.subdomain}</p>

						{#if school.subdomainUrl || (school.config && school.config.deployment_url)}
							<p class="subdomain-url">
								<strong>URL:</strong>
								<a
									href={school.subdomainUrl || school.config.deployment_url}
									target="_blank"
									rel="noopener noreferrer"
								>
									{school.subdomainUrl || school.config.deployment_url}
									<span class="external-link">↗</span>
								</a>
							</p>
						{/if}

						<p><strong>Database:</strong> {school.dbName}</p>
						<p>
							<strong>Created:</strong>
							{new Date(school.createdAt).toLocaleDateString('th-TH')}
						</p>

						{#if school.status === 'deployment_failed' && school.config && school.config.error}
							<div class="error-message">
								<strong>ข้อผิดพลาด:</strong>
								{school.config.error}
							</div>
						{/if}
					</div>

					<div class="school-actions">
						<a href="/dashboard/schools/{school.id}" class="btn-secondary btn-sm"> ดูรายละเอียด </a>
						<button
							class="btn-primary btn-sm"
							onclick={() => handleDeploy(school.id, school.name)}
							disabled={deploying === school.id ||
								school.status !== 'active' ||
								school.deploymentStatus === 'provisioning'}
						>
							{deploying === school.id ? '🔄 Deploying...' : '🚀 Deploy'}
						</button>

						<button
							class="btn-danger btn-sm"
							onclick={() => deleteSchool(school.id, school.name)}
							disabled={deploying === school.id || school.deploymentStatus === 'provisioning'}
						>
							🗑️ ลบ
						</button>
					</div>
				</div>
			{/each}
		</div>

		<div class="pagination">
			<button onclick={prevPage} disabled={page === 1}>← ก่อนหน้า</button>
			<span>หน้า {page} จาก {totalPages} (ทั้งหมด {total} โรงเรียน)</span>
			<button onclick={nextPage} disabled={page === totalPages}>ถัดไป →</button>
		</div>
	{/if}
</div>
```

<style>
	.schools-page {
		padding: 2rem;
		max-width: 1400px;
		margin: 0 auto;
	}
	
	.header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 2rem;
	}
	
	h1 {
		font-size: 2rem;
		font-weight: 700;
		color: #1a202c;
	}
	
	.btn-primary {
		background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
		color: white;
		border: none;
		padding: 0.75rem 1.5rem;
		border-radius: 8px;
		font-weight: 600;
		cursor: pointer;
		transition: transform 0.2s;
	}
	
	.btn-primary:hover:not(:disabled) {
		transform: translateY(-2px);
		box-shadow: 0 8px 16px rgba(102, 126, 234, 0.3);
	}
	
	.btn-primary:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}
	
	.btn-secondary {
		background: white;
		color: #667eea;
		border: 2px solid #667eea;
	}
	
	.btn-danger {
		background: #e53e3e;
		color: white;
		border: none;
	}
	
	.btn-sm {
		padding: 0.5rem 1rem;
		border-radius: 6px;
		font-size: 0.875rem;
		font-weight: 600;
		cursor: pointer;
		transition: all 0.2s;
	}
	
	.full-width {
		width: 100%;
	}
	
	.alert {
		padding: 1rem;
		border-radius: 8px;
		margin-bottom: 1rem;
	}
	
	.alert-error {
		background: #fed7d7;
		color: #c53030;
		border: 1px solid #fc8181;
	}
	
	.create-form-card {
		background: white;
		padding: 2rem;
		border-radius: 12px;
		box-shadow: 0 4px 16px rgba(0, 0, 0, 0.1);
		margin-bottom: 2rem;
	}
	
	.create-form-card h2 {
		margin-bottom: 1.5rem;
		color: #2d3748;
	}
	
	.form-group {
		margin-bottom: 1.5rem;
	}
	
	label {
		display: block;
		margin-bottom: 0.5rem;
		font-weight: 600;
		color: #4a5568;
	}
	
	input {
		width: 100%;
		padding: 0.75rem;
		border: 2px solid #e2e8f0;
		border-radius: 8px;
		font-size: 1rem;
		transition: border-color 0.2s;
	}
	
	input:focus {
		outline: none;
		border-color: #667eea;
	}
	
	small {
		display: block;
		margin-top: 0.25rem;
		color: #718096;
		font-size: 0.875rem;
	}
	
	.loading {
		text-align: center;
		padding: 4rem;
		color: #718096;
	}
	
	.empty-state {
		text-align: center;
		padding: 4rem;
		color: #a0aec0;
	}
	
	.schools-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(350px, 1fr));
		gap: 1.5rem;
		margin-bottom: 2rem;
	}
	
	.school-card {
		background: white;
		padding: 1.5rem;
		border-radius: 12px;
		box-shadow: 0 4px 16px rgba(0, 0, 0, 0.1);
		transition: transform 0.2s;
	}
	
	.school-card:hover {
		transform: translateY(-4px);
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.15);
	}
	
	.school-header {
		display: flex;
		justify-content: space-between;
		align-items: start;
		margin-bottom: 1rem;
	}
	
	.school-header h3 {
		font-size: 1.25rem;
		color: #2d3748;
		margin: 0;
		flex: 1;
	}
	
	.status-badges {
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}
	
	.status {
		padding: 0.25rem 0.75rem;
		border-radius: 12px;
		font-size: 0.75rem;
		font-weight: 600;
		text-transform: uppercase;
		background: #e2e8f0;
		color: #4a5568;
		display: inline-flex;
		align-items: center;
		gap: 0.25rem;
	}
	
	.status.active {
		background: #c6f6d5;
		color: #22543d;
	}
	
	.status.provisioning {
		background: #bee3f8;
		color: #2c5282;
		animation: pulse 2s ease-in-out infinite;
	}
	
	.status.failed {
		background: #fed7d7;
		color: #c53030;
	}
	
	@keyframes pulse {
		0%, 100% {
			opacity: 1;
		}
		50% {
			opacity: 0.7;
		}
	}
	
	.spinner {
		width: 12px;
		height: 12px;
		border: 2px solid #2c5282;
		border-top-color: transparent;
		border-radius: 50%;
		display: inline-block;
		animation: spin 0.8s linear infinite;
	}
	
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
	
	.school-info {
		margin-bottom: 1rem;
	}
	
	.school-info p {
		margin: 0.5rem 0;
		color: #4a5568;
		font-size: 0.875rem;
	}
	
	.subdomain-url a {
		color: #667eea;
		text-decoration: none;
		font-weight: 600;
		display: inline-flex;
		align-items: center;
		gap: 0.25rem;
		transition: color 0.2s;
	}
	
	.subdomain-url a:hover {
		color: #764ba2;
		text-decoration: underline;
	}
	
	.external-link {
		font-size: 0.75rem;
	}
	
	.error-message {
		margin-top: 0.75rem;
		padding: 0.75rem;
		background: #fed7d7;
		color: #c53030;
		border-radius: 6px;
		font-size: 0.875rem;
		border-left: 3px solid #e53e3e;
	}
	
	.school-actions {
		display: flex;
		gap: 0.5rem;
	}
	
	.school-actions a,
	.school-actions button {
		flex: 1;
		text-align: center;
		text-decoration: none;
	}
	
	.pagination {
		display: flex;
		justify-content: center;
		align-items: center;
		gap: 1rem;
		margin-top: 2rem;
	}
	
	.pagination button {
		padding: 0.5rem 1rem;
		border: 2px solid #e2e8f0;
		background: white;
		border-radius: 8px;
		cursor: pointer;
		font-weight: 600;
		transition: all 0.2s;
	}
	
	.pagination button:hover:not(:disabled) {
		border-color: #667eea;
		color: #667eea;
	}
	
	.pagination button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
	
	.btn-delete {
		padding: 0.5rem 1rem;
		background: #ef4444;
		color: white;
		border: none;
		border-radius: 6px;
		cursor: pointer;
		transition: background 0.2s;
	}
	
	.btn-delete:hover:not(:disabled) {
		background: #dc2626;
	}
	
	.btn-delete:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
	
	.btn-deploy {
		padding: 0.5rem 1rem;
		background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
		color: white;
		border: none;
		border-radius: 6px;
		cursor: pointer;
		font-weight: 600;
		transition: transform 0.2s, box-shadow 0.2s;
	}
	
	.btn-deploy:hover:not(:disabled) {
		transform: translateY(-2px);
		box-shadow: 0 4px 12px rgba(102, 126, 234, 0.4);
	}
	
	.btn-deploy:disabled {
		opacity: 0.6;
		cursor: not-allowed;
		transform: none;
	}
	
	.pagination span {
		color: #4a5568;
		font-weight: 500;
	}
