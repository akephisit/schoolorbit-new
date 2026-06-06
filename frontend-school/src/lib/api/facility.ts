import { apiClient, type ApiResponse } from '$lib/api/client';

type EmptyResponseData = Record<string, never>;
type LoadedApiResponse<T> = ApiResponse<T> & { success: true; data: T };

// Helper for authenticated requests
async function fetchApi<T = EmptyResponseData>(
	path: string,
	options: RequestInit = {}
): Promise<LoadedApiResponse<T>> {
	const method = (options.method || 'GET').toUpperCase();
	const body = options.body ? JSON.parse(options.body.toString()) : undefined;

	let response: ApiResponse<T>;
	if (method === 'POST') {
		response = await apiClient.post<T>(path, body);
	} else if (method === 'PUT') {
		response = await apiClient.put<T>(path, body);
	} else if (method === 'DELETE') {
		response = await apiClient.delete<T>(path);
	} else {
		response = await apiClient.get<T>(path);
	}

	if (!response.success) throw new Error(response.error || 'Request failed');
	if (response.data === undefined) throw new Error('Response data missing');
	return { success: true, data: response.data, message: response.message };
}

// Types
export interface Building {
	id: string;
	name_th: string;
	name_en?: string;
	code?: string;
	description?: string;
	created_at?: string;
	updated_at?: string;
}

export interface Room {
	id: string;
	building_id?: string;
	name_th: string;
	name_en?: string;
	code?: string;
	room_type: string;
	capacity: number;
	floor?: number;
	status: string;
	description?: string;

	// Joined
	building_name?: string;
}

export interface CreateBuildingRequest {
	name_th: string;
	name_en?: string;
	code?: string;
	description?: string;
}

export interface CreateRoomRequest {
	building_id?: string;
	name_th: string;
	name_en?: string;
	code?: string;
	room_type: string;
	capacity?: number;
	floor?: number;
	status?: string;
	description?: string;
}

// API Functions
const BASE = '/api/facilities';

export const listBuildings = async (): Promise<LoadedApiResponse<Building[]>> => {
	return await fetchApi<Building[]>(`${BASE}/buildings`);
};

export const createBuilding = async (
	data: CreateBuildingRequest
): Promise<LoadedApiResponse<Building>> => {
	return await fetchApi<Building>(`${BASE}/buildings`, {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const updateBuilding = async (
	id: string,
	data: Partial<CreateBuildingRequest>
): Promise<LoadedApiResponse<Building>> => {
	return await fetchApi<Building>(`${BASE}/buildings/${id}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

export const deleteBuilding = async (
	id: string
): Promise<LoadedApiResponse<Record<string, never>>> => {
	return await fetchApi<Record<string, never>>(`${BASE}/buildings/${id}`, { method: 'DELETE' });
};

export const listRooms = async (
	filters: {
		building_id?: string;
		room_type?: string;
		search?: string;
	} = {}
): Promise<LoadedApiResponse<Room[]>> => {
	const params = new URLSearchParams();
	if (filters.building_id) params.append('building_id', filters.building_id);
	if (filters.room_type) params.append('room_type', filters.room_type);
	if (filters.search) params.append('search', filters.search);

	const queryString = params.toString() ? `?${params.toString()}` : '';
	return await fetchApi<Room[]>(`${BASE}/rooms${queryString}`);
};

export const createRoom = async (data: CreateRoomRequest): Promise<LoadedApiResponse<Room>> => {
	return await fetchApi<Room>(`${BASE}/rooms`, {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const updateRoom = async (
	id: string,
	data: Partial<CreateRoomRequest>
): Promise<LoadedApiResponse<Room>> => {
	return await fetchApi<Room>(`${BASE}/rooms/${id}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

export const deleteRoom = async (id: string): Promise<LoadedApiResponse<Record<string, never>>> => {
	return await fetchApi<Record<string, never>>(`${BASE}/rooms/${id}`, { method: 'DELETE' });
};
