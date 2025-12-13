#!/bin/bash
# Script to verify admin user exists in the database

echo "🔍 Checking admin users in database..."

# Load environment variables
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
fi

# Check if DATABASE_URL is set
if [ -z "$DATABASE_URL" ]; then
    echo "❌ DATABASE_URL not set"
    exit 1
fi

# Query to check admin users
QUERY="SELECT id, national_id, name, role, created_at FROM admin_users ORDER BY created_at DESC LIMIT 5;"

echo "Running query: $QUERY"
echo ""

# Use psql to query the database
psql "$DATABASE_URL" -c "$QUERY"

echo ""
echo "✅ Query completed"
