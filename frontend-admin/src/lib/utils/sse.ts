export type LogLevel = 'info' | 'success' | 'error' | 'warning';

export interface SseMessage<T = unknown> {
	type: 'log' | 'progress' | 'complete' | 'error';
	level?: LogLevel;
	message?: string;
	step?: number;
	total?: number;
	data?: T;
	error?: string;
}

export interface LogMessage {
	level: LogLevel;
	message: string;
	timestamp: Date;
}

export interface Progress {
	step: number;
	total: number;
	message: string;
}

export interface SseCallbacks<T = unknown> {
	onLog?: (level: LogLevel, message: string) => void;
	onProgress?: (step: number, total: number, message: string) => void;
	onComplete?: (data: T) => void;
	onError?: (error: string) => void;
}

export function connectSSE<T = unknown>(
	url: string,
	options: RequestInit,
	callbacks: SseCallbacks<T>
): EventSource {
	// For POST requests with SSE, we need to use fetch + EventSource workaround
	// Since EventSource only supports GET, we'll use a different approach

	const eventSource = new EventSource(url);

	eventSource.onmessage = (event) => {
		try {
			const msg = JSON.parse(event.data) as SseMessage<T>;

			switch (msg.type) {
				case 'log':
					if (msg.level && msg.message) {
						callbacks.onLog?.(msg.level, msg.message);
					}
					break;

				case 'progress':
					if (msg.step !== undefined && msg.total !== undefined && msg.message) {
						callbacks.onProgress?.(msg.step, msg.total, msg.message);
					}
					break;

				case 'complete':
					callbacks.onComplete?.(msg.data as T);
					eventSource.close();
					break;

				case 'error':
					if (msg.error) {
						callbacks.onError?.(msg.error);
					}
					eventSource.close();
					break;
			}
		} catch (err) {
			console.error('Failed to parse SSE message:', err);
		}
	};

	eventSource.onerror = (error) => {
		console.error('SSE connection error:', error);
		callbacks.onError?.('Connection lost');
		eventSource.close();
	};

	return eventSource;
}

// Helper for POST SSE requests
export async function createSchoolSSE<T = unknown>(
	apiUrl: string,
	data: unknown,
	callbacks: SseCallbacks<T>
): Promise<void> {
	const response = await fetch(`${apiUrl}/api/v1/schools/stream`, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json'
		},
		body: JSON.stringify(data),
		credentials: 'include'
	});

	if (!response.ok) {
		throw new Error(`Failed to start SSE: ${response.statusText}`);
	}

	const reader = response.body?.getReader();
	if (!reader) {
		throw new Error('No response body');
	}

	const decoder = new TextDecoder();
	let buffer = '';

	try {
		while (true) {
			const { done, value } = await reader.read();

			if (done) break;

			buffer += decoder.decode(value, { stream: true });
			const lines = buffer.split('\n');
			buffer = lines.pop() || '';

			for (const line of lines) {
				if (line.startsWith('data: ')) {
					const data = line.slice(6);
					try {
						const msg = JSON.parse(data) as SseMessage<T>;

						switch (msg.type) {
							case 'log':
								if (msg.level && msg.message) {
									callbacks.onLog?.(msg.level, msg.message);
								}
								break;

							case 'progress':
								if (msg.step !== undefined && msg.total !== undefined && msg.message) {
									callbacks.onProgress?.(msg.step, msg.total, msg.message);
								}
								break;

							case 'complete':
								callbacks.onComplete?.(msg.data as T);
								return;

							case 'error':
								if (msg.error) {
									callbacks.onError?.(msg.error);
								}
								return;
						}
					} catch (err) {
						console.error('Failed to parse SSE message:', err);
					}
				}
			}
		}
	} finally {
		reader.releaseLock();
	}
}

// Helper for DELETE SSE requests
export async function deleteSchoolSSE(
	apiUrl: string,
	schoolId: string,
	callbacks: SseCallbacks
): Promise<void> {
	const response = await fetch(`${apiUrl}/api/v1/schools/${schoolId}/stream`, {
		method: 'DELETE',
		credentials: 'include'
	});

	if (!response.ok) {
		throw new Error(`Failed to start SSE: ${response.statusText}`);
	}

	const reader = response.body?.getReader();
	if (!reader) {
		throw new Error('No response body');
	}

	const decoder = new TextDecoder();
	let buffer = '';

	try {
		while (true) {
			const { done, value } = await reader.read();

			if (done) break;

			buffer += decoder.decode(value, { stream: true });
			const lines = buffer.split('\n');
			buffer = lines.pop() || '';

			for (const line of lines) {
				if (line.startsWith('data: ')) {
					const data = line.slice(6);
					try {
						const msg: SseMessage = JSON.parse(data);

						switch (msg.type) {
							case 'log':
								if (msg.level && msg.message) {
									callbacks.onLog?.(msg.level, msg.message);
								}
								break;

							case 'progress':
								if (msg.step !== undefined && msg.total !== undefined && msg.message) {
									callbacks.onProgress?.(msg.step, msg.total, msg.message);
								}
								break;

							case 'complete':
								callbacks.onComplete?.(msg.data);
								return;

							case 'error':
								if (msg.error) {
									callbacks.onError?.(msg.error);
								}
								return;
						}
					} catch (err) {
						console.error('Failed to parse SSE message:', err);
					}
				}
			}
		}
	} finally {
		reader.releaseLock();
	}
}
