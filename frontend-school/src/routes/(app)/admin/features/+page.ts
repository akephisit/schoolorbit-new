/**
 * Feature Toggles Management Page
 */

export const meta = {
    menu: {
        title: 'จัดการระบบงาน',
        icon: 'Zap',
        group: 'settings',
        order: 1000,
        permission: 'settings'
    }
};

export const load = async () => {
    return {
        title: 'จัดการระบบงาน'
    };
};
