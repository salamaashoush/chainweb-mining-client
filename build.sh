#!/bin/bash
set -e

#
# This script builds and pushes a multi-architecture Docker image for the
# chainweb-mining-client.
#
# Usage:
#   ./build.sh <your-docker-hub-username> [tag]
#
# Example:
#   ./build.sh my-docker-user latest
#

# --- Configuration ---

# The Docker Hub username or registry to push the image to.
DOCKER_USERNAME=${1:-"salamaashoush"}
IMAGE_NAME="chainweb-mining-client"
# Use the provided tag or default to "latest"
TAG=${2:-latest}

# The platforms to build for.
PLATFORMS="linux/amd64,linux/arm64"

# --- Script ---

if [ -z "$DOCKER_USERNAME" ]; then
  echo "Error: Docker username not provided."
  echo "Usage: ./build.sh <your-docker-hub-username> [tag]"
  exit 1
fi

# Get the version from chainweb-mining-client.cabal
VERSION=$(sed -n 's/^version:[ \t]*\([^ \t]*\).*/\1/p' chainweb-mining-client.cabal)

# The full image tag.
FULL_IMAGE_TAG="${DOCKER_USERNAME}/${IMAGE_NAME}:${TAG}"
VERSION_TAG="${DOCKER_USERNAME}/${IMAGE_NAME}:${VERSION}"
echo "Building and pushing multi-arch image: ${FULL_IMAGE_TAG}"
echo "Platforms: ${PLATFORMS}"

# Ensure the buildx builder is available.
# This will create a new builder if one doesn't exist.
if ! docker buildx ls | grep -q "multi-arch-builder"; then
  echo "Creating new buildx builder: multi-arch-builder"
  docker buildx create --name multi-arch-builder --use
fi

# Build and push the image.
docker buildx build \
  --platform "${PLATFORMS}" \
  --tag "${FULL_IMAGE_TAG}" \
  --tag "${VERSION_TAG}" \
  --push \
  .

echo "Successfully pushed ${FULL_IMAGE_TAG}"
