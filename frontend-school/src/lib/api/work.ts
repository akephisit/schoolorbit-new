import { apiClient, requireApiData, type ApiResponse } from '$lib/api/client';

export type WorkflowWindowStatus = 'draft' | 'open' | 'closed' | 'archived';
export type WorkflowWindowTimeState =
	| 'draft'
	| 'scheduled'
	| 'open'
	| 'due_soon'
	| 'overdue'
	| 'closed'
	| 'archived';

export interface WorkflowWindowMetadata {
	tags: string[];
}

export interface WorkflowWindow {
	id: string;
	moduleCode: string;
	workflowCode: string;
	title: string;
	description?: string | null;
	organizationUnitId?: string | null;
	managedByPermission: string;
	opensAt?: string | null;
	dueAt?: string | null;
	closesAt?: string | null;
	status: WorkflowWindowStatus;
	timeState: WorkflowWindowTimeState;
	metadata: WorkflowWindowMetadata;
	createdBy?: string | null;
	createdAt: string;
	updatedAt: string;
}

export interface ListWorkflowWindowsParams {
	moduleCode?: string;
	status?: WorkflowWindowStatus;
}

export interface CreateWorkflowWindowRequest {
	moduleCode: string;
	workflowCode: string;
	title: string;
	description?: string | null;
	organizationUnitId?: string | null;
	managedByPermission: string;
	opensAt?: string | null;
	dueAt?: string | null;
	closesAt?: string | null;
	metadata?: WorkflowWindowMetadata;
}

export type WorkItemLifecycleStatus = 'active' | 'closed' | 'cancelled' | 'archived';
export type WorkItemAssigneeType = 'user' | 'organization_unit' | 'organization_position';
export type WorkItemAssigneeStatus = 'assigned' | 'read' | 'submitted' | 'dismissed';
export type WorkItemState =
	| 'scheduled'
	| 'open'
	| 'due_soon'
	| 'overdue'
	| 'submitted'
	| 'closed'
	| 'archived';

export interface WorkItemMetadata {
	tags: string[];
	sourceLabel?: string | null;
}

export interface WorkItem {
	id: string;
	workflowWindowId: string;
	moduleCode: string;
	workflowCode: string;
	sourceResourceType: string;
	sourceResourceId?: string | null;
	title: string;
	description?: string | null;
	actionPath: string;
	requiredPermission?: string | null;
	itemStatus: WorkItemLifecycleStatus;
	assigneeId: string;
	assigneeType: WorkItemAssigneeType;
	assigneeStatus: WorkItemAssigneeStatus;
	state: WorkItemState;
	opensAt?: string | null;
	dueAt?: string | null;
	closesAt?: string | null;
	readAt?: string | null;
	submittedAt?: string | null;
	metadata: WorkItemMetadata;
	createdAt: string;
	updatedAt: string;
}

export interface WorkItemCounts {
	open: number;
	dueSoon: number;
	overdue: number;
	submitted: number;
	closed: number;
	total: number;
}

export interface ListWorkItemsParams {
	moduleCode?: string;
	state?: WorkItemState;
}

export interface CreateWorkItemAssigneeTarget {
	assigneeType: WorkItemAssigneeType;
	userId?: string | null;
	organizationUnitId?: string | null;
	positionCode?: string | null;
}

export interface CreateWorkItemRequest {
	workflowWindowId: string;
	moduleCode: string;
	sourceResourceType: string;
	sourceResourceId?: string | null;
	title: string;
	description?: string | null;
	actionPath: string;
	requiredPermission?: string | null;
	metadata?: WorkItemMetadata;
	assignees: CreateWorkItemAssigneeTarget[];
}

function workItemsQuery(params: ListWorkItemsParams = {}): string {
	const search = new URLSearchParams();
	if (params.moduleCode) search.set('moduleCode', params.moduleCode);
	if (params.state) search.set('state', params.state);
	const query = search.toString();
	return query ? `?${query}` : '';
}

function workflowWindowsQuery(params: ListWorkflowWindowsParams = {}): string {
	const search = new URLSearchParams();
	if (params.moduleCode) search.set('moduleCode', params.moduleCode);
	if (params.status) search.set('status', params.status);
	const query = search.toString();
	return query ? `?${query}` : '';
}

export async function listManageableWorkflowWindows(
	params: ListWorkflowWindowsParams = {}
): Promise<WorkflowWindow[]> {
	const response = await apiClient.get<{ items: WorkflowWindow[] }>(
		`/api/me/workflow-windows/manageable${workflowWindowsQuery(params)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดรอบงานที่จัดการได้').items;
}

export async function createWorkflowWindow(
	payload: CreateWorkflowWindowRequest
): Promise<ApiResponse<WorkflowWindow>> {
	return apiClient.post<WorkflowWindow>('/api/workflow-windows', payload);
}

export async function updateWorkflowWindowStatus(
	id: string,
	status: WorkflowWindowStatus
): Promise<ApiResponse<WorkflowWindow>> {
	return apiClient.patch<WorkflowWindow>(`/api/workflow-windows/${id}`, { status });
}

export async function getMyWorkItems(params: ListWorkItemsParams = {}): Promise<WorkItem[]> {
	const response = await apiClient.get<{ items: WorkItem[] }>(
		`/api/me/work-items${workItemsQuery(params)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดรายการงานได้').items;
}

export async function getMyWorkCounts(): Promise<WorkItemCounts> {
	const response = await apiClient.get<WorkItemCounts>('/api/me/work-items/counts');
	return requireApiData(response, 'ไม่สามารถโหลดจำนวนงานได้');
}

export async function createWorkItem(
	payload: CreateWorkItemRequest
): Promise<ApiResponse<{ id: string }>> {
	return apiClient.post<{ id: string }>('/api/work-items', payload);
}
