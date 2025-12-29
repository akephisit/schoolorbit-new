/**
 * Staff Management Page
 */

export const _meta = {
    menu: {
        title: 'บุคลากร',
        icon: 'Users',
        group: 'hr',
        order: 10,
        permission: 'staff'
    }
};

export const load = async () => {
    return {
        title: 'จัดการบุคลากร'
    };
};
