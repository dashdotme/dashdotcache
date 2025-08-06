#!/usr/bin/env -S nix shell nixpkgs#curl nixpkgs#jq --command bash

BASE_URL="http://127.0.0.1:8080"

echo "🚀 Testing Dashdotcache REST API"
echo "================================="

echo "1. Testing PING..."
curl -s -X POST "$BASE_URL/ping" \
  -H "Content-Type: application/json" \
  -d '{}' | jq '.'

echo -e "\n2. Setting key 'hello' = 'world'..."
curl -s -X POST "$BASE_URL/keys/hello" \
  -H "Content-Type: application/json" \
  -d '{"value": "world"}' | jq '.'

echo -e "\n3. Getting key 'hello'..."
curl -s -X GET "$BASE_URL/keys/hello" | jq '.'

echo -e "\n4. Setting key 'temp' with 60s TTL..."
curl -s -X POST "$BASE_URL/keys/temp" \
  -H "Content-Type: application/json" \
  -d '{"value": "temporary", "ttl": 60}' | jq '.'

echo -e "\n5. Getting TTL for 'temp'..."
curl -s -X GET "$BASE_URL/keys/temp/ttl" | jq '.'

echo -e "\n6. Setting expiration on 'hello' (30 seconds)..."
curl -s -X POST "$BASE_URL/keys/hello/expire" \
  -H "Content-Type: application/json" \
  -d '{"seconds": 30}' | jq '.'

echo -e "\n7. Getting full info for 'hello'..."
curl -s -X GET "$BASE_URL/keys/hello/info" | jq '.'

echo -e "\n8. Checking if keys exist..."
curl -s -X POST "$BASE_URL/keys/exists" \
  -H "Content-Type: application/json" \
  -d '{"keys": ["hello", "temp", "nonexistent"]}' | jq '.'

echo -e "\n9. Deleting key 'temp'..."
curl -s -X DELETE "$BASE_URL/keys/temp" | jq '.'

echo -e "\n10. Trying to get deleted key 'temp'..."
curl -s -X GET "$BASE_URL/keys/temp" | jq '.'

echo -e "\n11. Setting multiple keys for bulk delete test..."
curl -s -X POST "$BASE_URL/keys/test1" \
  -H "Content-Type: application/json" \
  -d '{"value": "value1"}' > /dev/null

curl -s -X POST "$BASE_URL/keys/test2" \
  -H "Content-Type: application/json" \
  -d '{"value": "value2"}' > /dev/null

curl -s -X POST "$BASE_URL/keys/test3" \
  -H "Content-Type: application/json" \
  -d '{"value": "value3"}' > /dev/null

echo "Bulk deleting test1, test2, test3..."
curl -s -X DELETE "$BASE_URL/keys" \
  -H "Content-Type: application/json" \
  -d '{"keys": ["test1", "test2", "test3"]}' | jq '.'

echo -e "\n12. Getting cache stats..."
curl -s -X GET "$BASE_URL/stats" | jq '.'

echo -e "\n13. Getting metrics..."
curl -s -X GET "$BASE_URL/metrics"

echo -e "\n14. Getting dashboard..."
curl -s -X GET "$BASE_URL/dash" | jq '.'

echo -e "\n15. Persisting key 'hello' (removing TTL)..."
curl -s -X POST "$BASE_URL/keys/hello/persist" | jq '.'

echo -e "\n✅ Tests completed!"
echo "Note: Some placeholder endpoints will return 'not implemented' errors - this is expected."
