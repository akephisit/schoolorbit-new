import { writable, type Writable } from 'svelte/store';
import { PUBLIC_BACKEND_URL } from '$env/static/public';

// Types matching backend
export interface UserPresence {
    user_id: string;
    name: string;
    color: string;
}

export type TimetableEvent =
    | { type: 'UserJoined', payload: UserPresence }
    | { type: 'UserLeft', payload: { user_id: string } }
    | { type: 'CursorMove', payload: { user_id: string, x: number, y: number, day?: string, period_id?: string } }
    // Locking
    | { type: 'DragStart', payload: { user_id: string, course_id?: string, entry_id?: string } }
    | { type: 'DragEnd', payload: { user_id: string } };

// Stores
export const activeUsers: Writable<UserPresence[]> = writable([]);
export const remoteCursors: Writable<Record<string, { x: number, y: number, day?: string, period_id?: string }>> = writable({});
// Key: user_id -> What they are dragging
export const userDrags: Writable<Record<string, { course_id?: string, entry_id?: string }>> = writable({});
export const isConnected: Writable<boolean> = writable(false);

let socket: WebSocket | null = null;
let currentUserId: string | null = null;

export function connectTimetableSocket(params: {
    school_id: string,
    semester_id: string,
    user_id: string,
    name: string
}) {
    if (socket) {
        socket.close();
    }
    currentUserId = params.user_id;

    let baseUrl = PUBLIC_BACKEND_URL || 'http://localhost:8081';
    let wsUrl = baseUrl.replace(/^http/, 'ws');

    // Ensure school_id/semester_id are strings
    const safeParams = {
        ...params,
        school_id: String(params.school_id),
        semester_id: String(params.semester_id),
        user_id: String(params.user_id)
    };

    const qs = new URLSearchParams(safeParams).toString();
    const url = `${wsUrl}/ws/timetable?${qs}`;

    console.log('Connecting to WS:', url);
    socket = new WebSocket(url);

    socket.onopen = () => {
        console.log('WS Connected');
        isConnected.set(true);
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
    };

    socket.onerror = (err) => {
        console.error('WS Error', err);
    };
}

export function disconnectTimetableSocket() {
    if (socket) {
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
            const { user_id, x, y, day, period_id } = payload;
            if (user_id === currentUserId) return;

            remoteCursors.update(cursors => ({
                ...cursors,
                [user_id]: { x, y, day, period_id }
            }));
            break;
        }
        case 'DragStart': {
            const { user_id, course_id, entry_id } = payload;
            if (user_id === currentUserId) return;

            userDrags.update(drags => ({
                ...drags,
                [user_id]: { course_id, entry_id }
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
    }
}
