#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Setting up cross-compilation environment for Linux on macOS...${NC}"

# Check if we're on macOS
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo -e "${RED}Error: This script is designed for macOS only${NC}"
    exit 1
fi

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check if Homebrew is installed
if ! command_exists brew; then
    echo -e "${RED}Error: Homebrew is required but not installed${NC}"
    echo "Please install Homebrew first: https://brew.sh/"
    exit 1
fi

# Check if Rust is installed
if ! command_exists rustc; then
    echo -e "${RED}Error: Rust is required but not installed${NC}"
    echo "Please install Rust first: https://rustup.rs/"
    exit 1
fi

echo -e "${YELLOW}Step 1: Installing cross-compilation toolchain...${NC}"

# Install cross-compilation tools
brew install filosottile/musl-cross/musl-cross || true

# Install LLVM for better cross-compilation support
brew install llvm || true

echo -e "${YELLOW}Step 2: Adding Linux target to Rust...${NC}"

# Add the Linux target
rustup target add x86_64-unknown-linux-gnu

echo -e "${YELLOW}Step 3: Installing cross tool...${NC}"

# Install cross for easier cross-compilation
cargo install cross --git https://github.com/cross-rs/cross

echo -e "${YELLOW}Step 4: Setting up cargo configuration...${NC}"

# Create .cargo directory if it doesn't exist
mkdir -p .cargo

# Create cargo config for cross-compilation
cat > .cargo/config.toml << 'EOF'
[target.x86_64-unknown-linux-gnu]
linker = "x86_64-linux-gnu-gcc"

[env]
CC_x86_64_unknown_linux_gnu = "x86_64-linux-gnu-gcc"
CXX_x86_64_unknown_linux_gnu = "x86_64-linux-gnu-g++"
AR_x86_64_unknown_linux_gnu = "x86_64-linux-gnu-ar"
CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER = "x86_64-linux-gnu-gcc"

# OpenCV specific settings for cross-compilation
[target.x86_64-unknown-linux-gnu.env]
PKG_CONFIG_ALLOW_CROSS = "1"
PKG_CONFIG_PATH = "/usr/lib/x86_64-linux-gnu/pkgconfig"
OPENCV_LINK_LIBS = "opencv_core,opencv_imgproc,opencv_imgcodecs,opencv_highgui,opencv_video,opencv_videoio"
OPENCV_LINK_PATHS = "/usr/lib/x86_64-linux-gnu"
OPENCV_INCLUDE_PATHS = "/usr/include/opencv4"

# Disable default OpenCV features that might cause issues
OPENCV_DISABLE_PROBES = "1"
EOF

echo -e "${YELLOW}Step 5: Installing Linux cross-compilation toolchain...${NC}"

# Try to install cross-compilation toolchain
if command_exists apt-get; then
    echo "Using apt-get to install cross-compilation tools..."
    sudo apt-get update
    sudo apt-get install -y gcc-x86_64-linux-gnu g++-x86_64-linux-gnu
elif command_exists brew; then
    echo "Installing cross-compilation tools via Homebrew..."

    # Install cross-compilation toolchain
    brew tap SergioBenitez/osxct
    brew install x86_64-unknown-linux-gnu || true

    # Alternative: Install a more complete toolchain
    brew install FiloSottile/musl-cross/musl-cross || true

    # If the above doesn't work, try installing individual components
    if ! command_exists x86_64-linux-gnu-gcc; then
        echo -e "${YELLOW}Installing alternative cross-compilation toolchain...${NC}"
        brew install messense/macos-cross-toolchains/x86_64-unknown-linux-gnu || true
    fi
fi

echo -e "${YELLOW}Step 6: Creating build helper scripts...${NC}"

# Create a build script that uses cross
cat > build-linux.sh << 'EOF'
#!/bin/bash

set -e

echo "Building for Linux using cross..."

# Build using cross (which uses Docker internally)
cross build --target x86_64-unknown-linux-gnu --release

if [ -f "target/x86_64-unknown-linux-gnu/release/dips_alt" ]; then
    echo "✓ Build successful!"
    echo "Linux binary: target/x86_64-unknown-linux-gnu/release/dips_alt"
    file target/x86_64-unknown-linux-gnu/release/dips_alt
else
    echo "✗ Build failed - binary not found"
    exit 1
fi
EOF

# Create alternative build script for manual toolchain
cat > build-linux-manual.sh << 'EOF'
#!/bin/bash

set -e

echo "Building for Linux using manual toolchain..."

# Set environment variables for cross-compilation
export CC=x86_64-linux-gnu-gcc
export CXX=x86_64-linux-gnu-g++
export AR=x86_64-linux-gnu-ar
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc

# Disable OpenCV probes to avoid macOS library detection
export OPENCV_DISABLE_PROBES=1

# Build the project
cargo build --target x86_64-unknown-linux-gnu --release

if [ -f "target/x86_64-unknown-linux-gnu/release/dips_alt" ]; then
    echo "✓ Build successful!"
    echo "Linux binary: target/x86_64-unknown-linux-gnu/release/dips_alt"
    file target/x86_64-unknown-linux-gnu/release/dips_alt
else
    echo "✗ Build failed - binary not found"
    exit 1
fi
EOF

# Make scripts executable
chmod +x build-linux.sh
chmod +x build-linux-manual.sh

echo -e "${YELLOW}Step 7: Creating OpenCV cross-compilation workaround...${NC}"

# Create a script to handle OpenCV dependencies
cat > fix-opencv-cross.sh << 'EOF'
#!/bin/bash

# This script helps with OpenCV cross-compilation issues

echo "Setting up OpenCV for cross-compilation..."

# Option 1: Disable OpenCV features that are problematic
export OPENCV_DISABLE_PROBES=1
export OPENCV_DISABLE_PKG_CONFIG=1

# Option 2: Use a minimal OpenCV configuration
export OPENCV_LINK_LIBS="opencv_core,opencv_imgproc,opencv_imgcodecs"

# Option 3: Build without OpenCV (if needed)
# You can modify Cargo.toml to make OpenCV optional:
# opencv = { version = "0.94.3", features = ["clang-runtime"], optional = true }

echo "OpenCV configuration set for cross-compilation"
echo "You may need to modify your Cargo.toml to make OpenCV optional if cross-compilation continues to fail"
EOF

chmod +x fix-opencv-cross.sh

echo -e "${GREEN}✓ Cross-compilation setup complete!${NC}"
echo
echo -e "${BLUE}Usage:${NC}"
echo "1. First, try building with cross (recommended):"
echo "   ./build-linux.sh"
echo
echo "2. If that fails, try the manual approach:"
echo "   ./build-linux-manual.sh"
echo
echo "3. If OpenCV causes issues, run:"
echo "   ./fix-opencv-cross.sh"
echo
echo -e "${YELLOW}Note:${NC} If you continue to have issues with OpenCV, consider:"
echo "- Using Docker-based compilation (cross uses Docker internally)"
echo "- Making OpenCV an optional dependency in your Cargo.toml"
echo "- Building on a Linux system or VM"
echo
echo -e "${BLUE}Troubleshooting:${NC}"
echo "- If linker errors persist, ensure x86_64-linux-gnu-gcc is in your PATH"
echo "- For OpenCV issues, you may need to build in a Linux environment"
echo "- Check that Docker is installed and running for the cross tool"
