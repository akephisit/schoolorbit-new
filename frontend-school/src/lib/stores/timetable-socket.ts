import { writable, type Writable } from 'svelte/store';
import { PUBLIC_BACKEND_URL } from '$env/static/public';

// Types matching backend
export interface UserContext {
    view_mode: string;
    view_id?: string;
}

export interface DragInfo {
    code: string;
    title: string;
    color?: string;
}

export interface UserPresence {
    user_id: string;
    name: string;
    color: string;
    context?: UserContext;
}

export type TimetableEvent =
    | { type: 'StateSync', payload: { users: UserPresence[], drags: Record<string, { course_id?: string, entry_id?: string, info?: DragInfo }> } }
    | { type: 'TableRefresh', payload: { user_id: string } }
    | { type: 'UserJoined', payload: UserPresence }
    | { type: 'UserLeft', payload: { user_id: string } }
    | { type: 'CursorMove', payload: { user_id: string, x: number, y: number, context?: UserContext } }
    // Locking
    | { type: 'DragStart', payload: { user_id: string, course_id?: string, entry_id?: string, info?: DragInfo } }
    | { type: 'DragEnd', payload: { user_id: string } };

// Stores
export const activeUsers: Writable<UserPresence[]> = writable([]);
export const remoteCursors: Writable<Record<string, { x: number, y: number, context?: UserContext }>> = writable({});
// Key: user_id -> What they are dragging
export const userDrags: Writable<Record<string, { course_id?: string, entry_id?: string, info?: DragInfo }>> = writable({});
export const refreshTrigger: Writable<number> = writable(0);
export const isConnected: Writable<boolean> = writable(false);

let socket: WebSocket | null = null;
let currentUserId: string | null = null;
let reconnectTimer: any = null;
let shouldReconnect = false;
let lastParams: any = null;

export function connectTimetableSocket(params: {
    semester_id: string,
    user_id: string,
    name: string
}) {
    // If we are already connected with same params, do nothing?
    // Or force reconnect? Let's force reconnect to be safe but debounce?

    // Check duplicate connection
    if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
        if (lastParams &&
            String(lastParams.semester_id) === String(params.semester_id) &&
            String(lastParams.user_id) === String(params.user_id)
        ) {
            // Same params, already connected/connecting
            return;
        }
    }

    // Clear any pending reconnect
    if (reconnectTimer) clearTimeout(reconnectTimer);
    shouldReconnect = true;
    lastParams = params;

    if (socket) {
        // Prevent old socket from triggering reconnect
        socket.onclose = null;
        socket.onerror = null;
        socket.close();
    }
    currentUserId = params.user_id;

    let baseUrl = PUBLIC_BACKEND_URL || 'http://localhost:8081';
    let wsUrl = baseUrl.replace(/^http/, 'ws');

    // Auto-detect school_key from hostname
    let schoolKey = 'default';
    if (typeof window !== 'undefined') {
        const parts = window.location.hostname.split('.');
        if (parts.length >= 3) {
            schoolKey = parts[0];
        }
    }

    // Ensure semester_id/user_id are strings
    const safeParams = {
        ...params,
        semester_id: String(params.semester_id),
        user_id: String(params.user_id),
        school_key: schoolKey
    };

    const qs = new URLSearchParams(safeParams).toString();
    const url = `${wsUrl}/ws/timetable?${qs}`;

    console.log('Connecting to WS:', url);
    socket = new WebSocket(url);

    socket.onopen = () => {
        console.log('WS Connected');
        isConnected.set(true);
        // Clear reconnect timer if any (redundant but safe)
        if (reconnectTimer) clearTimeout(reconnectTimer);
    };

    socket.onmessage = (event) => {
        try {
            const msg = JSON.parse(event.data);
            handleMessage(msg);
        } catch (e) { console.error('WS Parse Error', e); }
    };

    socket.onclose = () => {
        console.log('WS Disconnected');
        isConnected.set(false);
        // Clear state
        activeUsers.set([]);
        remoteCursors.set({});
        userDrags.set({});

        // Auto Reconnect
        if (shouldReconnect) {
            console.log('Attempting to reconnect in 3s...');
            reconnectTimer = setTimeout(() => {
                if (shouldReconnect && lastParams) {
                    connectTimetableSocket(lastParams);
                }
            }, 3000);
        }
    };

    socket.onerror = (err) => {
        console.error('WS Error', err);
        // On error, onclose usually fires too, but just in case
    };
}

export function disconnectTimetableSocket() {
    shouldReconnect = false;
    if (reconnectTimer) clearTimeout(reconnectTimer);

    if (socket) {
        // Prevent any pending callbacks
        socket.onclose = null;
        socket.onerror = null;
        socket.close();
        socket = null;
    }
}

export function sendTimetableEvent(event: TimetableEvent) {
    if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify(event));
    }
}

function handleMessage(msg: any) {
    const { type, payload } = msg;

    switch (type) {
        case 'StateSync': {
            const { users, drags } = payload;
            // Filter out self
            const others = users.filter((u: UserPresence) => u.user_id !== currentUserId);
            activeUsers.set(others);

            // Sync drags (filter self if necessary, but usually drag store is by user_id ok)
            if (currentUserId && drags[currentUserId]) {
                delete drags[currentUserId];
            }
            userDrags.set(drags);
            break;
        }
        case 'UserJoined': {
            const user = payload as UserPresence;
            if (user.user_id === currentUserId) return; // Ignore reflection if any

            // Add if not exists
            activeUsers.update(users => {
                if (users.find(u => u.user_id === user.user_id)) return users;
                return [...users, user];
            });
            break;
        }
        case 'UserLeft': {
            const { user_id } = payload;
            activeUsers.update(users => users.filter(u => u.user_id !== user_id));

            // Allow cleanup of cursor/drag
            remoteCursors.update(cursors => {
                // copy and delete
                const newCursors = { ...cursors };
                delete newCursors[user_id];
                return newCursors;
            });
            userDrags.update(drags => {
                const newDrags = { ...drags };
                delete newDrags[user_id];
                return newDrags;
            });
            break;
        }
        case 'CursorMove': {
            const { user_id, x, y, context } = payload;
            if (user_id === currentUserId) return;

            // Update user context in activeUsers list too?
            activeUsers.update(users => users.map(u =>
                u.user_id === user_id ? { ...u, context } : u
            ));

            remoteCursors.update(cursors => ({
                ...cursors,
                [user_id]: { x, y, context }
            }));
            break;
        }
        case 'DragStart': {
            const { user_id, course_id, entry_id, info } = payload;
            if (user_id === currentUserId) return;

            userDrags.update(drags => ({
                ...drags,
                [user_id]: { course_id, entry_id, info }
            }));
            break;
        }
        case 'DragEnd': {
            const { user_id } = payload;
            if (user_id === currentUserId) return;

            userDrags.update(drags => {
                const newDrags = { ...drags };
                delete newDrags[user_id];
                return newDrags;
            });
            break;
        }
        case 'TableRefresh': {
            const { user_id } = payload;
            // Always refresh to ensure consistent state across tabs/devices
            console.log('Received TableRefresh signal');
            refreshTrigger.update(n => n + 1);
            break;
        }
    }
}
