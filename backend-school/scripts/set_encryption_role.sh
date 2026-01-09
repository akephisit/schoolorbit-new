#!/bin/bash
#
# Script to set encryption key at database role level
# This makes the encryption key persistent for all sessions
#
# Usage: ./set_encryption_role.sh
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}🔐 Setting Encryption Key at Database Role Level${NC}"
echo ""

# Get encryption key from environment
if [ -z "$ENCRYPTION_KEY" ]; then
    echo -e "${RED}❌ ERROR: ENCRYPTION_KEY environment variable not set${NC}"
    echo "Please set it first:"
    echo "  export ENCRYPTION_KEY='your-key-here'"
    exit 1
fi

# Get database user from environment or use default
DB_USER=${DB_USER:-"your_db_user"}
echo -e "Database user: ${GREEN}$DB_USER${NC}"

# Get admin database URL
if [ -z "$ADMIN_DATABASE_URL" ]; then
    echo -e "${RED}❌ ERROR: ADMIN_DATABASE_URL not set${NC}"
    exit 1
fi

echo ""
echo -e "${YELLOW}📊 Fetching tenant databases...${NC}"

# Get list of tenant database URLs from admin database
TENANT_DBS=$(psql "$ADMIN_DATABASE_URL" -t -c "SELECT database_url FROM schools WHERE status = 'active';")

if [ -z "$TENANT_DBS" ]; then
    echo -e "${RED}❌ No active tenant databases found${NC}"
    exit 1
fi

# Count databases
DB_COUNT=$(echo "$TENANT_DBS" | wc -l)
echo -e "Found ${GREEN}$DB_COUNT${NC} active tenant database(s)"
echo ""

# Counter for success/failure
SUCCESS_COUNT=0
FAIL_COUNT=0

# Process each tenant database
while IFS= read -r TENANT_DB_URL; do
    # Extract database name from URL for display
    DB_NAME=$(echo "$TENANT_DB_URL" | sed 's/.*\///')
    
    echo -e "${YELLOW}Processing: $DB_NAME${NC}"
    
    # Set encryption key at role level
    if psql "$TENANT_DB_URL" -c "ALTER ROLE $DB_USER SET app.encryption_key = '$ENCRYPTION_KEY';" >/dev/null 2>&1; then
        echo -e "  ${GREEN}✅ Encryption key set successfully${NC}"
        ((SUCCESS_COUNT++))
    else
        echo -e "  ${RED}❌ Failed to set encryption key${NC}"
        ((FAIL_COUNT++))
    fi
    
done <<< "$TENANT_DBS"

echo ""
echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}✅ Success: $SUCCESS_COUNT${NC}"
if [ $FAIL_COUNT -gt 0 ]; then
    echo -e "${RED}❌ Failed: $FAIL_COUNT${NC}"
fi
echo ""

if [ $FAIL_COUNT -eq 0 ]; then
    echo -e "${GREEN}🎉 All databases configured successfully!${NC}"
    echo ""
    echo -e "${YELLOW}Next steps:${NC}"
    echo "1. Remove after_connect hook from pool_manager.rs (optional cleanup)"
    echo "2. Restart backend"
    echo "3. Encryption key will be set automatically for all connections!"
else
    echo -e "${RED}⚠️  Some databases failed. Please check errors above.${NC}"
    exit 1
fi
