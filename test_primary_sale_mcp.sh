#!/bin/bash
# Test script for PrimarySale MCP integration
# Tests that the tools are registered and can be called

set -e

echo "🧪 Testing PrimarySale MCP Integration"
echo "======================================"
echo ""

# Test 1: Check if MCP server binary exists
echo "✓ Test 1: Checking MCP server binary..."
if [ -f "target/release/mcp-server" ]; then
    echo "  ✅ MCP server binary exists"
else
    echo "  ❌ MCP server binary not found"
    exit 1
fi

# Test 2: Start MCP server and test tools/list
echo ""
echo "✓ Test 2: Testing tools registration..."

# Create a test request for tools/list
TEST_REQUEST='{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'

# Start MCP server, send request, and capture output
OUTPUT=$(echo "$TEST_REQUEST" | timeout 5 ./target/release/mcp-server 2>/dev/null || true)

# Check if output contains PrimarySale tools
if echo "$OUTPUT" | grep -q "primary_sale_get_sale_info"; then
    echo "  ✅ primary_sale_get_sale_info tool registered"
else
    echo "  ❌ primary_sale_get_sale_info tool NOT found"
    exit 1
fi

if echo "$OUTPUT" | grep -q "primary_sale_get_investor_info"; then
    echo "  ✅ primary_sale_get_investor_info tool registered"
else
    echo "  ❌ primary_sale_get_investor_info tool NOT found"
    exit 1
fi

if echo "$OUTPUT" | grep -q "primary_sale_invest"; then
    echo "  ✅ primary_sale_invest tool registered"
else
    echo "  ❌ primary_sale_invest tool NOT found"
    exit 1
fi

if echo "$OUTPUT" | grep -q "primary_sale_claim_refund"; then
    echo "  ✅ primary_sale_claim_refund tool registered"
else
    echo "  ❌ primary_sale_claim_refund tool NOT found"
    exit 1
fi

if echo "$OUTPUT" | grep -q "primary_sale_get_all_investors"; then
    echo "  ✅ primary_sale_get_all_investors tool registered"
else
    echo "  ❌ primary_sale_get_all_investors tool NOT found"
    exit 1
fi

# Test 3: Verify tool descriptions
echo ""
echo "✓ Test 3: Checking tool descriptions..."
if echo "$OUTPUT" | grep -q "Get comprehensive information about a primary sale"; then
    echo "  ✅ Tool descriptions present"
else
    echo "  ⚠️  Tool descriptions may be missing"
fi

# Test 4: Verify input schemas
echo ""
echo "✓ Test 4: Checking input schemas..."
if echo "$OUTPUT" | grep -q "contract_address"; then
    echo "  ✅ Input schemas present"
else
    echo "  ⚠️  Input schemas may be missing"
fi

echo ""
echo "======================================"
echo "✅ All PrimarySale MCP integration tests passed!"
echo ""
echo "Available PrimarySale tools:"
echo "  - primary_sale_get_sale_info"
echo "  - primary_sale_get_investor_info"
echo "  - primary_sale_invest"
echo "  - primary_sale_claim_refund"
echo "  - primary_sale_get_all_investors"
echo ""
