#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Cross-compiling DiPs for Linux...${NC}"

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to check if Docker is running
docker_running() {
    docker info >/dev/null 2>&1
}

# Method 1: Try using cross (Docker-based, most reliable)
try_cross() {
    echo -e "${YELLOW}Method 1: Attempting build with 'cross' tool...${NC}"

    if ! command_exists cross; then
        echo -e "${YELLOW}Installing cross tool...${NC}"
        cargo install cross --git https://github.com/cross-rs/cross
    fi

    if ! command_exists docker; then
        echo -e "${RED}Docker not found. Cross tool requires Docker.${NC}"
        return 1
    fi

    if ! docker_running; then
        echo -e "${RED}Docker is not running. Please start Docker and try again.${NC}"
        return 1
    fi

    echo -e "${YELLOW}Building with cross...${NC}"
    cross build --target x86_64-unknown-linux-gnu --release

    return $?
}

# Method 2: Try manual cross-compilation
try_manual() {
    echo -e "${YELLOW}Method 2: Attempting manual cross-compilation...${NC}"

    # Check if cross-compilation toolchain is available
    if ! command_exists x86_64-linux-gnu-gcc; then
        echo -e "${RED}Cross-compilation toolchain not found.${NC}"
        echo -e "${YELLOW}Install with: brew install FiloSottile/musl-cross/musl-cross${NC}"
        return 1
    fi

    # Set environment variables
    export CC=x86_64-linux-gnu-gcc
    export CXX=x86_64-linux-gnu-g++
    export AR=x86_64-linux-gnu-ar
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc

    # OpenCV cross-compilation settings
    export PKG_CONFIG_ALLOW_CROSS=1
    export OPENCV_DISABLE_PROBES=1
    export OPENCV_DISABLE_PKG_CONFIG=1

    echo -e "${YELLOW}Building with manual toolchain...${NC}"
    rustup target add x86_64-unknown-linux-gnu
    cargo build --target x86_64-unknown-linux-gnu --release

    return $?
}

# Method 3: Try with minimal OpenCV features
try_minimal_opencv() {
    echo -e "${YELLOW}Method 3: Attempting build with minimal OpenCV features...${NC}"

    # Backup original Cargo.toml
    cp dips_alt/Cargo.toml dips_alt/Cargo.toml.backup

    # Modify Cargo.toml to use minimal OpenCV features
    sed -i.tmp 's/opencv = { version = "0.94.3", features = \["clang-runtime"\] }/opencv = { version = "0.94.3", features = ["clang-runtime"], default-features = false }/' dips_alt/Cargo.toml

    # Try cross compilation again
    if try_cross; then
        echo -e "${GREEN}Build successful with minimal OpenCV features!${NC}"
        return 0
    fi

    # Restore original Cargo.toml
    mv dips_alt/Cargo.toml.backup dips_alt/Cargo.toml
    rm -f dips_alt/Cargo.toml.tmp

    return 1
}

# Method 4: Build without problematic dependencies
try_feature_flags() {
    echo -e "${YELLOW}Method 4: Attempting build with feature flags...${NC}"

    # Try building with specific feature combinations
    local features=(
        "--no-default-features"
        "--features minimal"
        "--features opencv-4"
    )

    for feature in "${features[@]}"; do
        echo -e "${YELLOW}Trying with: $feature${NC}"
        if cross build --target x86_64-unknown-linux-gnu --release $feature 2>/dev/null; then
            echo -e "${GREEN}Build successful with: $feature${NC}"
            return 0
        fi
    done

    return 1
}

# Main execution
main() {
    # Ensure we're in the right directory
    if [[ ! -f "Cargo.toml" ]]; then
        echo -e "${RED}Error: Not in a Cargo project directory${NC}"
        exit 1
    fi

    # Add Linux target if not already added
    rustup target add x86_64-unknown-linux-gnu >/dev/null 2>&1 || true

    # Try different methods in order of preference
    if try_cross; then
        echo -e "${GREEN}✓ Cross-compilation successful using 'cross' tool!${NC}"
    elif try_manual; then
        echo -e "${GREEN}✓ Cross-compilation successful using manual toolchain!${NC}"
    elif try_minimal_opencv; then
        echo -e "${GREEN}✓ Cross-compilation successful with minimal OpenCV!${NC}"
    elif try_feature_flags; then
        echo -e "${GREEN}✓ Cross-compilation successful with feature flags!${NC}"
    else
        echo -e "${RED}✗ All cross-compilation methods failed.${NC}"
        echo
        echo -e "${YELLOW}Troubleshooting suggestions:${NC}"
        echo "1. Install Docker and try again: brew install docker"
        echo "2. Install cross-compilation toolchain: brew install FiloSottile/musl-cross/musl-cross"
        echo "3. Consider building in a Linux VM or using GitHub Actions"
        echo "4. Make OpenCV optional in your Cargo.toml"
        echo
        echo -e "${BLUE}Alternative: Use Docker directly:${NC}"
        echo "docker run --rm -v \$(pwd):/workspace -w /workspace rust:latest cargo build --target x86_64-unknown-linux-gnu --release"
        exit 1
    fi

    # Check if binary was created
    BINARY_PATH="target/x86_64-unknown-linux-gnu/release/dips_alt"
    if [[ -f "$BINARY_PATH" ]]; then
        echo
        echo -e "${GREEN}Binary information:${NC}"
        file "$BINARY_PATH"
        ls -lh "$BINARY_PATH"
        echo
        echo -e "${GREEN}Linux binary available at: $BINARY_PATH${NC}"
    else
        echo -e "${RED}Warning: Binary not found at expected location${NC}"
        echo "Check target/x86_64-unknown-linux-gnu/release/ directory"
    fi
}

# Install dependencies if needed
install_deps() {
    echo -e "${YELLOW}Checking dependencies...${NC}"

    if ! command_exists rustup; then
        echo -e "${RED}Rust is required. Install from: https://rustup.rs/${NC}"
        exit 1
    fi

    if ! command_exists docker && ! command_exists cross; then
        echo -e "${YELLOW}Neither Docker nor cross-compilation toolchain found.${NC}"
        echo -e "${YELLOW}Attempting to install cross tool...${NC}"
        cargo install cross --git https://github.com/cross-rs/cross
    fi
}

# Show help
show_help() {
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  --help, -h    Show this help message"
    echo "  --install     Install required dependencies"
    echo "  --clean       Clean target directory before building"
    echo
    echo "This script attempts multiple cross-compilation methods:"
    echo "1. cross tool (Docker-based) - Most reliable"
    echo "2. Manual cross-compilation toolchain"
    echo "3. Minimal OpenCV features"
    echo "4. Alternative feature combinations"
}

# Parse command line arguments
case "${1:-}" in
    --help|-h)
        show_help
        exit 0
        ;;
    --install)
        install_deps
        exit 0
        ;;
    --clean)
        echo -e "${YELLOW}Cleaning target directory...${NC}"
        cargo clean
        ;;
esac

# Run main function
main
