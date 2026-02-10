# Docker Image Release Guide

## Overview

Production Docker images are built and signed via GitHub Actions with Sigstore attestation.
This proves images were built from this public repository, enabling Phala TEE verification.

## Building a Release

### From any branch

Tags can be created on any commit, not just main:

```bash
# From main branch
git checkout main
git tag v1.0.0
git push origin v1.0.0

# From feature branch
git checkout feature/my-changes
git tag v1.0.0-rc1
git push origin v1.0.0-rc1

# From specific commit
git tag v1.0.0 abc1234
git push origin v1.0.0
```

### Tag naming convention

- `v1.0.0` - Production release
- `v1.0.0-rc1` - Release candidate
- `v1.0.0-beta` - Beta release
- `v0.0.1-test` - Test release (for workflow testing)

## What happens on tag push

1. GitHub Actions builds `worker` and `keystore` images (linux/amd64)
2. Images are pushed to Docker Hub: `outlayer/near-outlayer-worker`, `outlayer/near-outlayer-keystore`
3. Sigstore attestation is created linking image digest to this repository
4. GitHub Release is created with digests and verification links

## Verifying Images

### Using GitHub CLI

```bash
gh attestation verify oci://docker.io/outlayer/near-outlayer-worker:v1.0.0 -R fastnear/near-outlayer
```

### Using Sigstore web

Visit: https://search.sigstore.dev/?hash=sha256:...

(Replace with actual digest from GitHub Release)

## Using Verified Images in Phala

Use SHA256 digest (from GitHub Release) instead of tags in docker-compose:

```yaml
# Instead of (mutable tag):
image: outlayer/near-outlayer-worker:v1.0.0

# Use (immutable digest):
image: docker.io/outlayer/near-outlayer-worker@sha256:abc123...
```

The digest ensures the exact same image is used, preventing tag mutation attacks.

## Local Development Builds

For quick iteration without Sigstore (not for production):

```bash
./scripts/build_and_push_phala.sh zavodil latest worker
./scripts/build_and_push_keystore_tee.sh zavodil latest
```

## GitHub Repository Setup

Before first release, configure GitHub repository:

**Variables** (Settings > Variables > Actions):
- `DOCKERHUB_USERNAME`: Your Docker Hub username or organization (e.g., `outlayer`)

**Secrets** (Settings > Secrets > Actions):
- `DOCKERHUB_TOKEN`: Docker Hub access token with Read & Write permissions
