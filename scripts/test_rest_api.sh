#!/usr/bin/env -S nix shell nixpkgs#curl nixpkgs#jq --command bash

# Helper function to handle responses that may or may not be JSON
handle_response() {
    local response="$1"
    # Try to parse as JSON, if it fails, just echo the response
    if echo "$response" | jq . >/dev/null 2>&1; then
        echo "$response" | jq .
    else
        echo "$response"
    fi
}

# The base URL of the running server
BASE_URL="http://127.0.0.1:8080"

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Testing Dashdotcache REST API at $BASE_URL${NC}"
echo "=============================================="

# ==============================================================================
# Setup
# ==============================================================================
echo -e "\n${GREEN}0. Flushing database for a clean slate...${NC}"
response=$(curl -s -X POST "$BASE_URL/flush")
handle_response "$response"

# ==============================================================================
# Core Commands
# ==============================================================================
echo -e "\n${GREEN}1. Testing PING...${NC}"
response=$(curl -s -X POST "$BASE_URL/ping" \
  -H "Content-Type: application/json" \
  -d '{"message": "Hello Server"}')
handle_response "$response"

echo -e "\n${GREEN}2. Setting key 'hello' = 'world'...${NC}"
response=$(curl -s -X POST "$BASE_URL/keys/hello" \
  -H "Content-Type: application/json" \
  -d '{"value": "world"}')
handle_response "$response"

echo -e "\n${GREEN}3. Getting key 'hello'...${NC}"
curl -s -X GET "$BASE_URL/keys/hello" | jq '.'

echo -e "\n${GREEN}4. Setting key 'temp' with 60s TTL...${NC}"
response=$(curl -s -X POST "$BASE_URL/keys/temp" \
  -H "Content-Type: application/json" \
  -d '{"value": "temporary", "ttl": 60}')
handle_response "$response"

echo -e "\n${GREEN}5. Getting TTL for 'temp'...${NC}"
curl -s -X GET "$BASE_URL/keys/temp/ttl" | jq '.'

echo -e "\n${GREEN}6. Getting full info for 'hello'...${NC}"
curl -s -X GET "$BASE_URL/keys/hello/info" | jq '.'

# ==============================================================================
# Relationship Commands (NEW)
# ==============================================================================
echo -e "\n${GREEN}7. Setting up parent/child relationships...${NC}"
echo "  - Creating keys: p1, c1, gc1"
curl -s -X POST "$BASE_URL/keys/p1" -H "Content-Type: application/json" -d '{"value": "parent"}' > /dev/null
curl -s -X POST "$BASE_URL/keys/c1" -H "Content-Type: application/json" -d '{"value": "child"}' > /dev/null
curl -s -X POST "$BASE_URL/keys/gc1" -H "Content-Type: application/json" -d '{"value": "grandchild"}' > /dev/null

echo "  - Setting c1's parent to p1..."
response=$(curl -s -X POST "$BASE_URL/keys/c1/parent" \
  -H "Content-Type: application/json" \
  -d '{"parent": "p1"}')
handle_response "$response"

echo "  - Setting gc1's parent to c1..."
response=$(curl -s -X POST "$BASE_URL/keys/gc1/parent" \
  -H "Content-Type: application/json" \
  -d '{"parent": "c1"}')
handle_response "$response"

echo -e "\n${GREEN}8. Getting immediate children of 'p1' (default depth)...${NC}"
curl -s -X GET "$BASE_URL/keys/p1/children" \
  -H "Content-Type: application/json" \
  -d '{}' | jq '.'

echo -e "\n${GREEN}9. Getting recursive children of 'p1' (depth=2)...${NC}"
curl -s -X GET "$BASE_URL/keys/p1/children" \
  -H "Content-Type: application/json" \
  -d '{"depth": 2}' | jq '.'

# ==============================================================================
# Listing and Bulk Commands
# ==============================================================================
echo -e "\n${GREEN}10. Setting up keys for listing (list:1, list:2)...${NC}"
curl -s -X POST "$BASE_URL/keys/list:1" -H "Content-Type: application/json" -d '{"value": "one"}' > /dev/null
curl -s -X POST "$BASE_URL/keys/list:2" -H "Content-Type: application/json" -d '{"value": "two"}' > /dev/null

echo -e "\n${GREEN}11. Listing keys with pattern 'list:*' (as query string)...${NC}"
curl -s -G "$BASE_URL/keys" \
  --data-urlencode "pattern=list:*" | jq '.'

echo -e "\n${GREEN}12. Listing keys with pattern 'list:*' and limit 1...${NC}"
curl -s -G "$BASE_URL/keys" \
  --data-urlencode "pattern=list:*" \
  --data-urlencode "limit=1" | jq '.'

echo -e "\n${GREEN}13. Checking if keys exist...${NC}"
curl -s -X POST "$BASE_URL/keys/exists" \
  -H "Content-Type: application/json" \
  -d '{"keys": ["hello", "temp", "p1", "nonexistent"]}' | jq '.'

echo -e "\n${GREEN}14. Deleting multiple keys...${NC}"
response=$(curl -s -X DELETE "$BASE_URL/keys" \
  -H "Content-Type: application/json" \
  -d '{"keys": ["list:1", "list:2", "temp"]}')
handle_response "$response"

# ==============================================================================
# Admin and Meta Commands
# ==============================================================================
echo -e "\n${GREEN}15. Getting cache stats...${NC}"
curl -s -X GET "$BASE_URL/stats"
echo ""

echo -e "\n${GREEN}16. Getting metrics...${NC}"
curl -s -X GET "$BASE_URL/metrics"
echo ""

echo -e "\n${RED}==============================================${NC}"
echo -e "${RED}✅ All tests completed!${NC}"
