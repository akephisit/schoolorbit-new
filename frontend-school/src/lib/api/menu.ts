// Menu API Client
// API for fetching user's dynamic menu based on permissions

export interface MenuItem {
    id: string;
    code: string;
    name: string;
    path: string;
    icon?: string;
}

export interface MenuGroup {
    code: string;
    name: string;
    icon?: string;
    items: MenuItem[];
}

export interface UserMenuResponse {
    success: boolean;
    groups: MenuGroup[];
}

/**
 * Fetch user's menu items based on their permissions
 * Menu is dynamically generated from database
 */
export async function getUserMenu(): Promise<UserMenuResponse> {
    const response = await fetch('/api/menu/user', {
        method: 'GET',
        credentials: 'include',
        headers: {
            'Content-Type': 'application/json'
        }
    });

    if (!response.ok) {
        throw new Error(`Failed to fetch menu: ${response.statusText}`);
    }

    return response.json();
}
