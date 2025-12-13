#!/bin/bash
# Quick diagnostic script for login issues

echo "🔍 SchoolOrbit Admin Login Diagnostics"
echo "========================================"
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test 1: Check if API is accessible
echo "1️⃣  Testing API accessibility..."
if curl -s -f https://admin-api.schoolorbit.app/health > /dev/null; then
    echo -e "${GREEN}✅ API is accessible${NC}"
else
    echo -e "${RED}❌ API is NOT accessible${NC}"
    exit 1
fi
echo ""

# Test 2: Check CORS headers
echo "2️⃣  Testing CORS configuration..."
CORS_RESPONSE=$(curl -s -I -X OPTIONS https://admin-api.schoolorbit.app/api/v1/auth/login \
  -H "Origin: https://admin.schoolorbit.app" \
  -H "Access-Control-Request-Method: POST")

if echo "$CORS_RESPONSE" | grep -q "Access-Control-Allow-Origin"; then
    echo -e "${GREEN}✅ CORS headers present${NC}"
    echo "$CORS_RESPONSE" | grep "Access-Control"
else
    echo -e "${RED}❌ CORS headers missing${NC}"
fi
echo ""

# Test 3: Try to login
echo "3️⃣  Testing login endpoint..."
LOGIN_RESPONSE=$(curl -s -w "\n%{http_code}" -X POST https://admin-api.schoolorbit.app/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -H "Origin: https://admin.schoolorbit.app" \
  -d '{"nationalId":"1234567890123","password":"test123"}')

HTTP_CODE=$(echo "$LOGIN_RESPONSE" | tail -n1)
RESPONSE_BODY=$(echo "$LOGIN_RESPONSE" | head -n-1)

echo "HTTP Status: $HTTP_CODE"
echo "Response: $RESPONSE_BODY"
echo ""

if [ "$HTTP_CODE" = "200" ]; then
    echo -e "${GREEN}✅ Login successful!${NC}"
    echo ""
    echo "You should now be able to login from the frontend."
elif [ "$HTTP_CODE" = "401" ]; then
    echo -e "${RED}❌ Login failed - Unauthorized${NC}"
    echo ""
    echo -e "${YELLOW}Possible causes:${NC}"
    echo "  1. Admin user doesn't exist in production database"
    echo "  2. Password hash doesn't match"
    echo "  3. Database connection issue"
    echo ""
    echo -e "${YELLOW}Solutions:${NC}"
    echo "  1. SSH into production server"
    echo "  2. cd /path/to/backend-admin"
    echo "  3. Run: ./scripts/create_prod_admin.sh"
    echo "  4. Restart backend service"
    echo ""
    echo -e "${YELLOW}Or check the full troubleshooting guide:${NC}"
    echo "  backend-admin/TROUBLESHOOTING_LOGIN.md"
else
    echo -e "${RED}❌ Unexpected HTTP status: $HTTP_CODE${NC}"
fi

echo ""
echo "========================================"
echo "🏁 Diagnostics complete"
