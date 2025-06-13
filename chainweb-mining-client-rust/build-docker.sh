#!/bin/bash
set -e

#
# This script builds and pushes multi-architecture Docker images for the
# Rust chainweb-mining-client.
#
# Usage:
#   ./build-docker.sh [tag] [dockerfile]
#
# Examples:
#   ./build-docker.sh                    # builds latest with Dockerfile
#   ./build-docker.sh v1.0.0            # builds v1.0.0 with Dockerfile
#   ./build-docker.sh latest distroless # builds latest with Dockerfile.distroless
#

# --- Configuration ---

# Use the provided tag or default to "latest"
TAG=${1:-latest}

# Use the provided dockerfile or default to "Dockerfile"
DOCKERFILE_TYPE=${2:-"scratch"}

# The platforms to build for
PLATFORMS="linux/amd64,linux/arm64"

# Image name
IMAGE_NAME="salamaashoush/chainweb-mining-client"

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

# Build the image locally first (for testing)
echo ""
echo "Building image locally for testing..."
docker buildx build \
    --platform "${PLATFORMS}" \
    --file "${DOCKERFILE}" \
    --tag "${IMAGE_NAME}${IMAGE_SUFFIX}:${TAG}" \
    --tag "${IMAGE_NAME}${IMAGE_SUFFIX}:${VERSION}" \
    --output "type=docker" \
    --push \
    .

echo ""
echo "Successfully built and pushed ${IMAGE_NAME}${IMAGE_SUFFIX}:${TAG}"
echo "Image sizes (approximate):"
docker images | grep "${IMAGE_NAME}" | head -5
