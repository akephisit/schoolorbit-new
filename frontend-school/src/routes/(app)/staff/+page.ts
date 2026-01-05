/**
 * Staff Dashboard
 * Main dashboard for staff members (no specific permission required)
 */

export const _meta = {
    menu: {
        title: 'แดชบอร์ด',
        icon: 'LayoutDashboard',
        group: 'main',
        order: 1
        // No permission required - all authenticated staff can access
    }
};

export const load = async () => {
    return {
        title: 'Staff Dashboard'
    };
};
