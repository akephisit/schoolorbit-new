#!/bin/bash
# Script to create/update admin user in the database
# 
# NOTE: The default admin user is automatically created by migration 005_seed_admin_user.sql
#       You don't need to run this script unless you want to:
#       1. Reset the default admin password
#       2. Create additional admin users (modify the create_admin binary)
#
# Default admin credentials (created by migration):
#   National ID: 1234567890123
#   Password: test123
#
# Run migration instead:
#   sqlx migrate run

echo "⚠️  NOTICE: Default admin user is created automatically by migration"
echo ""
echo "Default credentials:"
echo "  National ID: 1234567890123"
echo "  Password: test123"
echo ""
echo "If you need to reset the default admin password, proceed with this script."
echo "Otherwise, just run migrations: sqlx migrate run"
echo ""
read -p "Continue anyway? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Cancelled."
    exit 0
fi

echo "🔐 Creating/updating admin user..."

# Load environment variables
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
fi

# Compile and run the create_admin binary
cargo run --bin create_admin --release

echo "✅ Admin user creation completed"
echo ""
echo "Credentials:"
echo "  National ID: 1234567890123"
echo "  Password: test123"
