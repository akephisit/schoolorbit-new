import type { ActivityResult, SessionState } from './session-activity.js';

interface SharedStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
	removeItem(key: string): void;
}

export function readSharedSession(value: string | null): SessionState | null {
	if (!value) return null;
	let parsed: unknown;
	try {
		parsed = JSON.parse(value);
	} catch {
		return null;
	}
	if (!parsed || typeof parsed !== 'object') return null;
	if (
		!('sessionId' in parsed) ||
		typeof parsed.sessionId !== 'string' ||
		!/^[a-f0-9-]{36}$/i.test(parsed.sessionId)
	)
		return null;
	if (
		!('serverNow' in parsed) ||
		typeof parsed.serverNow !== 'string' ||
		!Number.isFinite(Date.parse(parsed.serverNow))
	)
		return null;
	if (
		!('idleExpiresAt' in parsed) ||
		typeof parsed.idleExpiresAt !== 'string' ||
		!Number.isFinite(Date.parse(parsed.idleExpiresAt))
	)
		return null;
	if (
		!('absoluteExpiresAt' in parsed) ||
		typeof parsed.absoluteExpiresAt !== 'string' ||
		!Number.isFinite(Date.parse(parsed.absoluteExpiresAt))
	)
		return null;
	if (
		!('nextRenewalAt' in parsed) ||
		typeof parsed.nextRenewalAt !== 'string' ||
		!Number.isFinite(Date.parse(parsed.nextRenewalAt))
	)
		return null;
	return {
		sessionId: parsed.sessionId,
		serverNow: parsed.serverNow,
		idleExpiresAt: parsed.idleExpiresAt,
		absoluteExpiresAt: parsed.absoluteExpiresAt,
		nextRenewalAt: parsed.nextRenewalAt
	};
}

export function sessionActivityKeys(origin: string, sessionId: string) {
	const namespace = `schoolorbit:activity:${origin}:${sessionId}`;
	return {
		metadata: `${namespace}:metadata`,
		lease: `${namespace}:lease`,
		lifecycle: `${namespace}:lifecycle`,
		lock: namespace
	};
}

interface CoordinationDependencies {
	storage: SharedStorage | null;
	locks?: {
		request<T>(
			name: string,
			options: { ifAvailable: boolean },
			callback: (lock: Lock | null) => Promise<T>
		): Promise<T>;
	};
	now(): number;
	random(): number;
	wait(delay: number, signal: AbortSignal): Promise<void>;
}

/** A short, best-effort browser lock. The server independently bounds all writes. */
export function createSessionActivityCoordinator(deps: CoordinationDependencies) {
	function read(key: string) {
		try {
			return deps.storage?.getItem(key) ?? null;
		} catch {
			return null;
		}
	}
	function write(key: string, value: string) {
		try {
			deps.storage?.setItem(key, value);
			return deps.storage !== null;
		} catch {
			return false;
		}
	}
	return {
		readSession(origin: string, sessionId: string) {
			const session = readSharedSession(read(sessionActivityKeys(origin, sessionId).metadata));
			return session?.sessionId === sessionId ? session : null;
		},
		publish(origin: string, session: SessionState) {
			write(sessionActivityKeys(origin, session.sessionId).metadata, JSON.stringify(session));
		},
		async run(
			origin: string,
			candidate: SessionState,
			signal: AbortSignal,
			renew: () => Promise<ActivityResult>
		): Promise<ActivityResult> {
			const keys = sessionActivityKeys(origin, candidate.sessionId);
			const execute = async (): Promise<ActivityResult> => {
				if (signal.aborted) return { kind: 'reconciled' };
				const shared = readSharedSession(read(keys.metadata));
				if (
					shared?.sessionId === candidate.sessionId &&
					Date.parse(shared.nextRenewalAt) > Date.parse(candidate.nextRenewalAt)
				)
					return { kind: 'renewed', session: shared };
				const result = await renew();
				if (result.kind === 'renewed' && !signal.aborted)
					write(keys.metadata, JSON.stringify(result.session));
				return result;
			};
			// Web Locks forbids combining ifAvailable with signal. The callback fences aborts.
			if (deps.locks)
				return deps.locks.request<ActivityResult>(keys.lock, { ifAvailable: true }, (lock) =>
					lock ? execute() : Promise.resolve({ kind: 'retry', retryAfterMs: 1_000 })
				);
			if (!deps.storage) return execute();
			const available = () => {
				const lease = read(keys.lease);
				if (!lease) return true;
				const expiry = Number(lease.split(':')[0]);
				return !Number.isFinite(expiry) || expiry <= deps.now() || expiry > deps.now() + 20_000;
			};
			await deps.wait(25 + deps.random() * 75, signal);
			if (!available()) return { kind: 'retry', retryAfterMs: 1_000 };
			const lease = `${deps.now() + 15_000}:${deps.random()}`;
			if (!write(keys.lease, lease)) return execute();
			await deps.wait(25 + deps.random() * 50, signal);
			if (read(keys.lease) !== lease) return { kind: 'retry', retryAfterMs: 1_000 };
			try {
				return await execute();
			} finally {
				if (read(keys.lease) === lease) {
					try {
						deps.storage.removeItem(keys.lease);
					} catch {
						/* A disabled store needs no cleanup. */
					}
				}
			}
		}
	};
}
