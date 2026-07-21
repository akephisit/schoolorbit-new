import { reconnectDelayMs } from './timetable-reconnect.ts';

const CONNECTION_DEBOUNCE_MS = 50;
const SOCKET_CONNECTING = 0;
const SOCKET_OPEN = 1;

export type TimetableSocketParams = {
	semester_id: string;
	current_user_id: string;
};

type TimetableSocketLike = {
	readyState: number;
	onopen: ((event: Event) => unknown) | null;
	onmessage: ((event: MessageEvent<unknown>) => unknown) | null;
	onclose: ((event: CloseEvent) => unknown) | null;
	onerror: ((event: Event) => unknown) | null;
	close(): void;
	send(data: string): void;
};

type TimetableSocketRuntimeDependencies<Timer> = {
	createSocket(params: TimetableSocketParams): TimetableSocketLike;
	setTimer(callback: () => void, delay: number): Timer;
	clearTimer(timer: Timer): void;
	isOnline(): boolean;
	addOnlineListener(listener: () => void): void;
	removeOnlineListener(listener: () => void): void;
	random?: () => number;
	onOpen(): void;
	onMessage(data: unknown): void;
	onClose(): void;
	onError(error: unknown): void;
};

export type TimetableSocketRuntime = {
	connect(params: TimetableSocketParams): void;
	disconnect(): void;
	send(data: string): boolean;
};

function sameParams(left: TimetableSocketParams | null, right: TimetableSocketParams): boolean {
	return (
		left !== null &&
		String(left.semester_id) === String(right.semester_id) &&
		String(left.current_user_id) === String(right.current_user_id)
	);
}

export function createTimetableSocketRuntime<Timer>(
	dependencies: TimetableSocketRuntimeDependencies<Timer>
): TimetableSocketRuntime {
	let socket: TimetableSocketLike | null = null;
	let activeParams: TimetableSocketParams | null = null;
	let desiredParams: TimetableSocketParams | null = null;
	let reconnectTimer: Timer | null = null;
	let connectionDebounceTimer: Timer | null = null;
	let shouldReconnect = false;
	let reconnectAttempt = 0;
	let waitingForOnline = false;
	let socketGeneration = 0;

	function clearReconnectTimer() {
		if (reconnectTimer === null) return;
		dependencies.clearTimer(reconnectTimer);
		reconnectTimer = null;
	}

	function clearConnectionDebounceTimer() {
		if (connectionDebounceTimer === null) return;
		dependencies.clearTimer(connectionDebounceTimer);
		connectionDebounceTimer = null;
	}

	function clearOnlineListener() {
		if (!waitingForOnline) return;
		dependencies.removeOnlineListener(handleOnline);
		waitingForOnline = false;
	}

	function detachSocketHandlers(target: TimetableSocketLike) {
		target.onopen = null;
		target.onmessage = null;
		target.onclose = null;
		target.onerror = null;
	}

	function retireSocket(target: TimetableSocketLike, close: boolean) {
		if (socket === target) {
			socket = null;
			activeParams = null;
			socketGeneration += 1;
		}

		detachSocketHandlers(target);
		if (close) target.close();
	}

	function ownsSocket(target: TimetableSocketLike, generation: number): boolean {
		return shouldReconnect && socket === target && socketGeneration === generation;
	}

	function handleOnline() {
		if (!waitingForOnline) return;
		clearOnlineListener();
		if (shouldReconnect && desiredParams) connect(desiredParams);
	}

	function scheduleReconnect() {
		if (!shouldReconnect || !desiredParams || connectionDebounceTimer !== null) return;
		if (!dependencies.isOnline()) {
			if (!waitingForOnline) {
				waitingForOnline = true;
				dependencies.addOnlineListener(handleOnline);
			}
			return;
		}
		if (reconnectTimer !== null) return;

		const delay = reconnectDelayMs(reconnectAttempt, dependencies.random);
		reconnectAttempt += 1;
		reconnectTimer = dependencies.setTimer(() => {
			reconnectTimer = null;
			if (shouldReconnect && desiredParams) connect(desiredParams);
		}, delay);
	}

	function openSocket(params: TimetableSocketParams) {
		if (socket) retireSocket(socket, true);

		let nextSocket: TimetableSocketLike;
		try {
			nextSocket = dependencies.createSocket(params);
		} catch (error) {
			dependencies.onError(error);
			scheduleReconnect();
			return;
		}

		socketGeneration += 1;
		const generation = socketGeneration;
		socket = nextSocket;
		activeParams = params;

		nextSocket.onopen = () => {
			if (!ownsSocket(nextSocket, generation)) return;
			reconnectAttempt = 0;
			clearReconnectTimer();
			clearOnlineListener();
			dependencies.onOpen();
		};

		nextSocket.onmessage = (event) => {
			if (!ownsSocket(nextSocket, generation)) return;
			dependencies.onMessage(event.data);
		};

		nextSocket.onclose = (event) => {
			if (!ownsSocket(nextSocket, generation)) return;
			retireSocket(nextSocket, false);
			dependencies.onClose();
			if (event.code === 1008) {
				shouldReconnect = false;
				desiredParams = null;
				clearReconnectTimer();
				clearConnectionDebounceTimer();
				clearOnlineListener();
				reconnectAttempt = 0;
				return;
			}
			scheduleReconnect();
		};

		nextSocket.onerror = (error) => {
			if (!ownsSocket(nextSocket, generation)) return;
			dependencies.onError(error);
		};
	}

	function connect(params: TimetableSocketParams) {
		const nextParams = { ...params };
		shouldReconnect = true;
		desiredParams = nextParams;
		clearReconnectTimer();
		clearOnlineListener();

		if (
			socket &&
			(socket.readyState === SOCKET_OPEN || socket.readyState === SOCKET_CONNECTING) &&
			sameParams(activeParams, nextParams)
		) {
			clearConnectionDebounceTimer();
			return;
		}

		clearConnectionDebounceTimer();
		connectionDebounceTimer = dependencies.setTimer(() => {
			connectionDebounceTimer = null;
			if (!shouldReconnect || !sameParams(desiredParams, nextParams)) return;
			openSocket(nextParams);
		}, CONNECTION_DEBOUNCE_MS);
	}

	function disconnect() {
		shouldReconnect = false;
		desiredParams = null;
		clearReconnectTimer();
		clearConnectionDebounceTimer();
		clearOnlineListener();
		reconnectAttempt = 0;
		if (socket) retireSocket(socket, true);
	}

	function send(data: string): boolean {
		if (!shouldReconnect || !socket || socket.readyState !== SOCKET_OPEN) return false;
		socket.send(data);
		return true;
	}

	return { connect, disconnect, send };
}
