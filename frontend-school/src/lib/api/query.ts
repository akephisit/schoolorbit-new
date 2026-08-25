export type ApiQueryPrimitive = string | number | boolean;
export type ApiQueryValue = ApiQueryPrimitive | readonly ApiQueryPrimitive[] | null | undefined;
export type ApiQuery = Readonly<Record<string, ApiQueryValue>>;

function appendValue(params: URLSearchParams, key: string, value: unknown): void {
	if (value === undefined || value === null) return;
	if (Array.isArray(value)) {
		for (const item of value) appendValue(params, key, item);
		return;
	}
	if (typeof value !== 'string' && typeof value !== 'number' && typeof value !== 'boolean') {
		throw new TypeError(`Unsupported API query value for ${key}`);
	}
	params.append(key, String(value));
}

export function appendApiQuery(endpoint: string, query?: ApiQuery): string {
	if (!query) return endpoint;
	const params = new URLSearchParams();
	for (const [key, value] of Object.entries(query)) appendValue(params, key, value);
	const encoded = params.toString();
	if (!encoded) return endpoint;
	return `${endpoint}${endpoint.includes('?') ? '&' : '?'}${encoded}`;
}
