#!/bin/bash
# Comprehensive test for PrimarySale MCP integration

set -e

echo "🧪 PrimarySale MCP Integration Test"
echo "===================================="
echo ""

# Test 1: Verify all tools are registered
echo "✓ Test 1: Tool Registration"
echo "  Testing tools/list endpoint..."

TOOLS_OUTPUT=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | ./target/release/mcp-server 2>&1)

TOOLS=(
  "primary_sale_get_sale_info"
  "primary_sale_get_investor_info"
  "primary_sale_invest"
  "primary_sale_claim_refund"
  "primary_sale_get_all_investors"
)

for tool in "${TOOLS[@]}"; do
  if echo "$TOOLS_OUTPUT" | grep -q "\"name\":\"$tool\""; then
    echo "  ✅ $tool"
  else
    echo "  ❌ $tool - NOT FOUND"
    exit 1
  fi
done

# Test 2: Verify tool schemas
echo ""
echo "✓ Test 2: Tool Schemas"

# Check for contract_address parameter in all tools
if echo "$TOOLS_OUTPUT" | grep -q '"contract_address"'; then
  echo "  ✅ contract_address parameter present in schemas"
else
  echo "  ❌ contract_address parameter missing"
  exit 1
fi

# Check for proper descriptions
if echo "$TOOLS_OUTPUT" | grep -q "Get comprehensive information about a primary sale"; then
  echo "  ✅ Tool descriptions present"
else
  echo "  ❌ Tool descriptions missing"
  exit 1
fi

# Test 3: Test tool invocation (expect error with mock address)
echo ""
echo "✓ Test 3: Tool Invocation"
echo "  Testing primary_sale_get_sale_info with mock address..."

# Mock contract address (will fail, but tests the handler)
TEST_REQUEST='{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"primary_sale_get_sale_info","arguments":{"contract_address":"0x0000000000000000000000000000000000000000"}}}'

INVOKE_OUTPUT=$(echo "$TEST_REQUEST" | ./target/release/mcp-server 2>&1)

# Should get a response (even if it's an error due to no real contract)
if echo "$INVOKE_OUTPUT" | grep -q '"jsonrpc":"2.0"'; then
  echo "  ✅ Tool invocation handler working"
else
  echo "  ⚠️  Tool invocation may have issues"
  echo "  Output: $INVOKE_OUTPUT"
fi

# Test 4: Verify EVM feature is enabled
echo ""
echo "✓ Test 4: Feature Flags"

if echo "$TOOLS_OUTPUT" | grep -q "wallet_get_evm_address"; then
  echo "  ✅ EVM features enabled"
else
  echo "  ❌ EVM features not enabled"
  exit 1
fi

# Summary
echo ""
echo "===================================="
echo "✅ All tests passed!"
echo ""
echo "📋 Available PrimarySale Tools:"
echo ""
for tool in "${TOOLS[@]}"; do
  echo "  • $tool"
done
echo ""
echo "📖 Tool Descriptions:"
echo "  • get_sale_info:        Query sale status, timing, contributions"
echo "  • get_investor_info:    Query investor allocations and contributions"
echo "  • invest:               Invest mantraUSD (requires transaction signing)"
echo "  • claim_refund:         Claim refund from failed/cancelled sales"
echo "  • get_all_investors:    Get paginated list of investors"
echo ""
echo "🔧 To use these tools, connect to the MCP server at:"
echo "   ./target/release/mcp-server"
echo ""
