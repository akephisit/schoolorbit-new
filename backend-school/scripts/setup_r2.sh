#!/bin/bash

# ============================================================================
# Cloudflare R2 Setup Helper Script
# ============================================================================
# This script helps you set up and configure Cloudflare R2 for SchoolOrbit
# ============================================================================

set -euo pipefail

echo "============================================"
echo "🚀 SchoolOrbit - R2 Setup Helper"
echo "============================================"
echo ""

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Function to print colored messages
info() {
    echo -e "${GREEN}ℹ${NC} $1"
}

warn() {
    echo -e "${YELLOW}⚠${NC} $1"
}

error() {
    echo -e "${RED}✗${NC} $1"
}

success() {
    echo -e "${GREEN}✓${NC} $1"
}

echo "This script will help you configure Cloudflare R2 for file storage."
echo ""

# ============================================================================
# Step 1: Check if .env file exists
# ============================================================================
if [ ! -f .env ]; then
    info "No .env file found. Creating from .env.example..."
    cp .env.example .env
    success ".env file created"
else
    info "Using existing .env file"
fi

echo ""
echo "============================================"
echo "📝 R2 Configuration"
echo "============================================"
echo ""
echo "You'll need the following from Cloudflare Dashboard:"
echo "  1. Account ID"
echo "  2. R2 Access Key ID"
echo "  3. R2 Secret Access Key"
echo "  4. Existing public bucket name"
echo "  5. New or existing private bucket name"
echo "  6. Public URL for the public bucket"
echo ""
echo "Get these from: https://dash.cloudflare.com"
echo "Navigate to: R2 Object Storage > Manage R2 API Tokens"
echo ""

read -p "Do you have your R2 credentials ready? (y/n): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    warn "Please get your R2 credentials first, then run this script again."
    exit 0
fi

# ============================================================================
# Step 2: Collect R2 credentials
# ============================================================================
echo ""
info "Enter your R2 credentials:"
echo ""

read -p "Account ID: " R2_ACCOUNT_ID
read -p "Access Key ID: " R2_ACCESS_KEY_ID
read -s -p "Secret Access Key: " R2_SECRET_ACCESS_KEY
echo ""
read -p "Existing public bucket [schoolorbit-public-files]: " R2_PUBLIC_BUCKET_NAME
R2_PUBLIC_BUCKET_NAME=${R2_PUBLIC_BUCKET_NAME:-schoolorbit-public-files}
read -p "Private bucket [schoolorbit-private-files]: " R2_PRIVATE_BUCKET_NAME
R2_PRIVATE_BUCKET_NAME=${R2_PRIVATE_BUCKET_NAME:-schoolorbit-private-files}

read -p "R2 Public URL (e.g., https://pub-xxxxx.r2.dev): " R2_PUBLIC_URL

if [ "$R2_PUBLIC_BUCKET_NAME" = "$R2_PRIVATE_BUCKET_NAME" ]; then
    error "Public and private bucket names must be different."
    exit 1
fi

# ============================================================================
# Step 3: Provision and verify the bucket topology
# ============================================================================
echo ""
if ! command -v aws &> /dev/null; then
    error "aws-cli is required to verify and provision the two-bucket topology."
    exit 1
fi

info "Checking configured R2 buckets without printing credentials..."
export AWS_ACCESS_KEY_ID="$R2_ACCESS_KEY_ID"
export AWS_SECRET_ACCESS_KEY="$R2_SECRET_ACCESS_KEY"
export AWS_DEFAULT_REGION=auto
R2_ENDPOINT="https://${R2_ACCOUNT_ID}.r2.cloudflarestorage.com"
bucket_list="$(aws --endpoint-url "$R2_ENDPOINT" s3api list-buckets --query 'Buckets[].Name' --output text)"

bucket_exists() {
    printf '%s\n' "$bucket_list" | tr '\t' '\n' | grep -Fqx -- "$1"
}

if ! bucket_exists "$R2_PUBLIC_BUCKET_NAME"; then
    error "The configured public bucket does not exist; it will not be created or replaced automatically."
    exit 1
fi
if ! bucket_exists "$R2_PRIVATE_BUCKET_NAME"; then
    info "The private bucket is absent; creating it without public access..."
    aws --endpoint-url "$R2_ENDPOINT" s3api create-bucket \
        --bucket "$R2_PRIVATE_BUCKET_NAME" >/dev/null
fi
aws --endpoint-url "$R2_ENDPOINT" s3api head-bucket \
    --bucket "$R2_PUBLIC_BUCKET_NAME" >/dev/null
aws --endpoint-url "$R2_ENDPOINT" s3api head-bucket \
    --bucket "$R2_PRIVATE_BUCKET_NAME" >/dev/null
unset AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY
success "Both R2 buckets are reachable."

# ============================================================================
# Step 4: Update .env file
# ============================================================================
echo ""
info "Updating .env file..."

# Function to update or add env variable
update_env() {
    local key=$1
    local value=$2
    
    if grep -q "^${key}=" .env; then
        # Update existing
        sed -i "s|^${key}=.*|${key}=${value}|" .env
    else
        # Add new
        echo "${key}=${value}" >> .env
    fi
}

update_env "R2_ACCOUNT_ID" "$R2_ACCOUNT_ID"
update_env "R2_ACCESS_KEY_ID" "$R2_ACCESS_KEY_ID"
update_env "R2_SECRET_ACCESS_KEY" "$R2_SECRET_ACCESS_KEY"
update_env "R2_PUBLIC_BUCKET_NAME" "$R2_PUBLIC_BUCKET_NAME"
update_env "R2_PRIVATE_BUCKET_NAME" "$R2_PRIVATE_BUCKET_NAME"
update_env "R2_PUBLIC_URL" "$R2_PUBLIC_URL"
update_env "R2_REGION" "auto"

success ".env file updated with R2 credentials"

# ============================================================================
# Step 5: Summary
# ============================================================================
echo ""
echo "============================================"
echo "✅ R2 Setup Complete!"
echo "============================================"
echo ""
success "R2 credentials configured in .env"
success "Public and private buckets verified"
success "Public URL: $R2_PUBLIC_URL"
echo ""
echo "Next steps:"
echo "  1. Start clamd and wait for its healthcheck"
echo "  2. Start backend-school and wait for /ready"
echo "  3. Run the File Platform smoke checks in docs/TESTING.md"
echo ""
echo "📖 For more information, see: ../docs/OPERATIONS.md"
echo ""
