#!/bin/sh
# Mock external miner for testing
# Simulates a simple external miner that reads work from stdin

echo "Mock miner started" >&2

while IFS= read -r line; do
    echo "Received work: $line" >&2
    
    # Parse the JSON work (simple extraction)
    target=$(echo "$line" | grep -o '"target":"[^"]*"' | cut -d'"' -f4)
    header=$(echo "$line" | grep -o '"header":"[^"]*"' | cut -d'"' -f4)
    
    if [ -n "$target" ] && [ -n "$header" ]; then
        echo "Target: $target" >&2
        echo "Header: $header" >&2
        
        # Simulate mining work (sleep)
        sleep 1
        
        # Return a mock solution
        echo '{"nonce": "0000000000000000"}'
    fi
done