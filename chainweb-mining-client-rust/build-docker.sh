#!/bin/bash
set -e

#
# This script builds and optionally pushes multi-architecture Docker images for the
# Rust chainweb-mining-client.
#
# Usage:
#   ./build-docker.sh [tag] [dockerfile] [--local]
#
# Examples:
#   ./build-docker.sh                    # builds and pushes latest with Dockerfile
#   ./build-docker.sh v1.0.0            # builds and pushes v1.0.0 with Dockerfile
#   ./build-docker.sh latest scratch --local # builds latest locally only (no push)
#

# --- Configuration ---

# Get the script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"

# Change to script directory
cd "$SCRIPT_DIR"

# Use the provided tag or default to "latest"
TAG=${1:-latest}

# Use the provided dockerfile or default to "Dockerfile"
DOCKERFILE_TYPE=${2:-"scratch"}

# Check if --local flag is present
LOCAL_ONLY=false
if [ "$3" = "--local" ] || [ "$2" = "--local" ]; then
    LOCAL_ONLY=true
    if [ "$2" = "--local" ]; then
        DOCKERFILE_TYPE="scratch"
    fi
fi

# The platforms to build for
if [ "$LOCAL_ONLY" = true ]; then
    # For local builds, only build for the current platform
    PLATFORMS="linux/$(uname -m | sed 's/x86_64/amd64/g')"
else
    PLATFORMS="linux/amd64,linux/arm64"
fi

# Image name
IMAGE_NAME="salamaashoush/chainweb-mining-client-rs"

# --- Script ---

# Determine which Dockerfile to use
if [ "$DOCKERFILE_TYPE" = "distroless" ]; then
    DOCKERFILE="Dockerfile.distroless"
    IMAGE_SUFFIX="-distroless"
else
    DOCKERFILE="Dockerfile"
    IMAGE_SUFFIX=""
fi

# Get the version from Cargo.toml
VERSION=$(grep "^version" Cargo.toml | head -1 | cut -d'"' -f2)

echo "Building multi-arch image using ${DOCKERFILE}"
echo "Version: ${VERSION}"
echo "Tag: ${TAG}"
echo "Platforms: ${PLATFORMS}"

# Ensure Docker buildx is set up
if ! docker buildx version > /dev/null 2>&1; then
    echo "Error: Docker buildx is not available. Please update Docker."
    exit 1
fi

# Create or use existing builder
BUILDER_NAME="chainweb-mining-builder"
if ! docker buildx ls | grep -q "$BUILDER_NAME"; then
    echo "Creating new buildx builder: $BUILDER_NAME"
    docker buildx create --name "$BUILDER_NAME" --driver docker-container --use
else
    echo "Using existing builder: $BUILDER_NAME"
    docker buildx use "$BUILDER_NAME"
fi

# Build the image
echo ""
if [ "$LOCAL_ONLY" = true ]; then
    echo "Building local image..."
    docker buildx build \
        --platform "${PLATFORMS}" \
        --file "${DOCKERFILE}" \
        --tag "${IMAGE_NAME}${IMAGE_SUFFIX}:${TAG}" \
        --tag "${IMAGE_NAME}${IMAGE_SUFFIX}:${VERSION}" \
        --load \
        .
    echo ""
    echo "Successfully built local image ${IMAGE_NAME}${IMAGE_SUFFIX}:${TAG}"
else
    echo "Building and pushing multi-arch image..."
    docker buildx build \
        --platform "${PLATFORMS}" \
        --file "${DOCKERFILE}" \
        --tag "${IMAGE_NAME}${IMAGE_SUFFIX}:${TAG}" \
        --tag "${IMAGE_NAME}${IMAGE_SUFFIX}:${VERSION}" \
        --push \
        .
    echo ""
    echo "Successfully built and pushed ${IMAGE_NAME}${IMAGE_SUFFIX}:${TAG}"
fi
echo "Image sizes (approximate):"
docker images | grep "${IMAGE_NAME}" | head -5
