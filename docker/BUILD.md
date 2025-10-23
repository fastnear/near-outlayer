# Multi-Platform Docker Build Guide

## Building WasmEdge Compiler Image for linux/amd64 and linux/arm64

### Step 1: Create buildx builder with docker-container driver
```bash
docker buildx create --name multiarch --driver docker-container --use
```

### Step 2: Bootstrap the builder
```bash
docker buildx inspect --bootstrap
```

### Step 3: Login to Docker Hub (if not already logged in)
```bash
docker login
```

### Step 4: Build and push multi-platform image
This single command builds for both amd64 and arm64, and pushes to Docker Hub:
```bash
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t zavodil/wasmedge-compiler:latest \
  -f docker/Dockerfile.wasmedge-compiler \
  --push \
  .
```

**Note:** The `--push` flag automatically pushes the image to Docker Hub after building.
No need for separate `docker push` command.

---

## Pulling the Image on Server

After pushing to Docker Hub, pull on your server:

```bash
# On Ubuntu server (amd64)
docker pull zavodil/wasmedge-compiler:latest

# Verify architecture
docker run --rm zavodil/wasmedge-compiler:latest uname -m
# Should output: x86_64
```

## Using the Image in Worker

Update `worker/.env`:
```bash
DOCKER_IMAGE=zavodil/wasmedge-compiler:latest
```

Restart worker:
```bash
cd worker
cargo run
```
