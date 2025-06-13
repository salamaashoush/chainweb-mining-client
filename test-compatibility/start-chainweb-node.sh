#!/bin/bash
# Start a Chainweb node with mining enabled for testing

set -e

# Configuration
NODE_TYPE="${1:-dev}"  # dev or prod
PUBLIC_KEY="f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Starting Chainweb node in $NODE_TYPE mode...${NC}"

if [ "$NODE_TYPE" = "dev" ]; then
    # Development node with PoW validation disabled
    docker run -d \
        --name chainweb-mining-test \
        -p 1848:1848 \
        -p 1789:1789 \
        -e DISABLE_POW_VALIDATION=1 \
        --ulimit nofile=65535:65535 \
        salamaashoush/chainweb-node:latest \
        /chainweb/chainweb-node \
        +RTS -T -H400M -A64M -RTS \
        --node-id=0 \
        --log-level=info \
        --enable-mining-coordination \
        --mining-public-key="$PUBLIC_KEY" \
        --header-stream \
        --allowReadsInLocal \
        --database-directory=/chainweb/db \
        --p2p-hostname=0.0.0.0 \
        --p2p-port=1789 \
        --service-port=1848 \
        --bootstrap-reachability=0 \
        --mempool-p2p-max-session-count=0 \
        --disable-mempool-p2p \
        --prune-chain-database=none \
        --fast-forward-block-height-limit=400
else
    # Production-like node with PoW validation enabled
    docker run -d \
        --name chainweb-mining-test \
        -p 1848:1848 \
        -p 1789:1789 \
        --ulimit nofile=65535:65535 \
        salamaashoush/chainweb-node:latest \
        /chainweb/chainweb-node \
        +RTS -T -H400M -A64M -RTS \
        --node-id=0 \
        --log-level=info \
        --enable-mining-coordination \
        --mining-public-key="$PUBLIC_KEY" \
        --header-stream \
        --allowReadsInLocal \
        --database-directory=/chainweb/db \
        --p2p-hostname=0.0.0.0 \
        --p2p-port=1789 \
        --service-port=1848 \
        --bootstrap-reachability=0 \
        --mempool-p2p-max-session-count=0 \
        --disable-mempool-p2p \
        --prune-chain-database=none
fi

echo -e "${GREEN}Chainweb node started!${NC}"
echo ""
echo "Node information:"
echo "  API endpoint: http://localhost:1848"
echo "  P2P port: 1789"
echo "  Mining public key: $PUBLIC_KEY"
echo "  Mode: $NODE_TYPE"
echo ""
echo "Commands:"
echo "  Check status:  curl http://localhost:1848/info | jq ."
echo "  View logs:     docker logs -f chainweb-mining-test"
echo "  Stop node:     docker stop chainweb-mining-test && docker rm chainweb-mining-test"
echo ""
echo "Waiting for node to be ready..."

# Wait for node to be ready
max_attempts=30
attempt=0
while [ $attempt -lt $max_attempts ]; do
    if curl -s http://localhost:1848/info >/dev/null 2>&1; then
        echo -e "${GREEN}Node is ready!${NC}"
        curl -s http://localhost:1848/info | jq '.nodeVersion'
        break
    fi
    sleep 2
    attempt=$((attempt + 1))
    echo -n "."
done

if [ $attempt -eq $max_attempts ]; then
    echo -e "${RED}Node failed to start. Check logs with: docker logs chainweb-mining-test${NC}"
    exit 1
fi