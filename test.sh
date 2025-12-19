#!/bin/bash
BASE_URL="${BASE_URL:-http://localhost:3000}"

echo "Testing BSV Ordinals Marketplace API at $BASE_URL"
echo ""

echo "1. Root endpoint:"
curl -s "$BASE_URL/" | jq .name
echo ""

echo "2. Health check:"
curl -s "$BASE_URL/health" | jq
echo ""

echo "To test wallet lookup:"
echo "  curl $BASE_URL/wallet/YOUR_BSV_ADDRESS | jq"
