#!/bin/bash
set -e

# Simple script to build Docker image locally for testing
# This builds only for the current platform and doesn't push

TAG=${1:-latest}

echo "Building local Docker image..."
echo "Tag: $TAG"
echo "Platform: linux/$(uname -m | sed 's/x86_64/amd64/g')"

./build-docker.sh "$TAG" scratch --local

echo ""
echo "To run the locally built image:"
echo "  docker run --rm salamaashoush/chainweb-mining-client-rs:$TAG --help"