import { apiClient, type ApiResponse, type ApiRequestOptions } from '#lib/api/client.js';

import type { components, operations } from '#lib/api/generated/school-api.js';

type Schemas = components['schemas'];
type EmptyResponseData = Schemas['EmptyData'];
type LoadedApiResponse<T> = ApiResponse<T> & { success: true; data: T };

// Helper for authenticated requests
async function fetchApi<T = EmptyResponseData>(
	path: string,
	options: RequestInit = {},
	requestOptions: ApiRequestOptions = {}
): Promise<LoadedApiResponse<T>> {
	const method = (options.method || 'GET').toUpperCase();
	const body = options.body ? JSON.parse(options.body.toString()) : undefined;

	let response: ApiResponse<T>;
	if (method === 'POST') {
		response = await apiClient.post<T>(path, body, requestOptions);
	} else if (method === 'PUT') {
		response = await apiClient.put<T>(path, body, requestOptions);
	} else if (method === 'DELETE') {
		response = await apiClient.delete<T>(path, requestOptions);
	} else {
		response = await apiClient.get<T>(path, requestOptions);
	}

	if (!response.success) throw new Error(response.error || 'Request failed');
	if (response.data === undefined) throw new Error('Response data missing');
	return { ...response, success: true, data: response.data };
}

export type Building = Schemas['Building'];
export type Room = Schemas['Room'];
export type CreateBuildingRequest = Schemas['CreateBuildingRequest'];
export type CreateRoomRequest = Schemas['CreateRoomRequest'];
type RoomFilters = NonNullable<operations['listFacilityRooms']['parameters']['query']>;

// API Functions
const BASE = '/api/facilities';

export const listBuildings = async (
	options: ApiRequestOptions = {}
): Promise<LoadedApiResponse<Building[]>> => {
	return await fetchApi<Building[]>(`${BASE}/buildings`, {}, options);
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
	data: Schemas['UpdateBuildingRequest']
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
	filters: RoomFilters = {},
	options: ApiRequestOptions = {}
): Promise<LoadedApiResponse<Room[]>> => {
	return await fetchApi<Room[]>(`${BASE}/rooms`, {}, { ...options, query: filters });
};

export const createRoom = async (data: CreateRoomRequest): Promise<LoadedApiResponse<Room>> => {
	return await fetchApi<Room>(`${BASE}/rooms`, {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const updateRoom = async (
	id: string,
	data: Schemas['UpdateRoomRequest']
): Promise<LoadedApiResponse<Room>> => {
	return await fetchApi<Room>(`${BASE}/rooms/${id}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

export const deleteRoom = async (id: string): Promise<LoadedApiResponse<Record<string, never>>> => {
	return await fetchApi<Record<string, never>>(`${BASE}/rooms/${id}`, { method: 'DELETE' });
};
