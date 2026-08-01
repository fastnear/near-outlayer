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
  -t outlayer/wasmedge-compiler:rust1.85-wasi25 \
  -f docker/Dockerfile.wasmedge-compiler \
  --push \
  .
```

**Note:** The `--push` flag automatically pushes the image to Docker Hub after building.
No need for separate `docker push` command.

### Deliberately no `latest` tag

There is none, and adding one would undo the point. `ensure_image` runs a pull on **every**
compile job, so a floating tag means anything pushed to this repository enters production
compilation with no deploy and no review. The image also decides which toolchain produces the
wasm whose hash the enclave attests — an unpinned compiler makes that measurement
irreproducible, which is the same reason every example commits its `Cargo.lock`.

Tag names carry the toolchain (`rust1.85-wasi25`) so a human can read them; deployments pin
the **digest**:

```bash
docker buildx imagetools inspect outlayer/wasmedge-compiler:rust1.85-wasi25
# -> Digest: sha256:...

# in the worker's env file
DOCKER_IMAGE=outlayer/wasmedge-compiler@sha256:5c996303f707381f463e591d7b0650e64816e63e7bc1d98ab72a62abf55b146e
```

The base image is digest-pinned in the Dockerfile for the same reason: `rust:1.85` is a moving
tag. Bump it deliberately.

---

## Pulling the Image on Server

After pushing to Docker Hub, pull on your server:

```bash
# On Ubuntu server (amd64)
docker pull outlayer/wasmedge-compiler:rust1.85-wasi25

# Verify architecture
docker run --rm outlayer/wasmedge-compiler:rust1.85-wasi25 uname -m
# Should output: x86_64
```

## Using the Image in Worker

Update `worker/.env`:
```bash
DOCKER_IMAGE=outlayer/wasmedge-compiler@sha256:5c996303f707381f463e591d7b0650e64816e63e7bc1d98ab72a62abf55b146e
```

Restart worker:
```bash
cd worker
cargo run
```
