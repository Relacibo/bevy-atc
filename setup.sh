#!/bin/bash
set -e

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR"

echo "🚁 Bevy ATC Setup Script"
echo "========================"
echo "Project root: $PROJECT_ROOT"

# Change to project root directory
cd "$PROJECT_ROOT"

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "crates" ]; then
    echo "❌ Error: Script must be in the bevy-atc root directory"
    echo "Current directory: $(pwd)"
    exit 1
fi

echo "📦 Initializing submodules..."
git submodule update --init --recursive

echo "🤖 Setting up ML models..."
MODELS_DIR="$PROJECT_ROOT/crates/atc_recognition_rs/resources/models/whisper-small.en-atc-experiment"

if [ -d "$MODELS_DIR" ]; then
    cd "$MODELS_DIR"
    echo "📥 Downloading whisper model (this may take a moment)..."
    
    # Skip LFS if already configured
    if [ ! -f "whisper-atc-q8_0.bin" ] || [ ! -s "whisper-atc-q8_0.bin" ]; then
        git lfs pull --include="whisper-atc-q8_0.bin"
        echo "✅ Model downloaded successfully!"
    else
        echo "✅ Model already exists and is valid"
    fi
    
    cd "$PROJECT_ROOT"
else
    echo "⚠️  Warning: Models directory not found. Submodule may not be initialized properly."
fi

echo "🔨 Building project..."
cd "$PROJECT_ROOT"
cargo build

echo ""
echo "🎉 Setup complete!"
echo ""
echo "To run the game:"
echo "  cargo run"
echo ""
echo "To update models later:"
echo "  ./scripts/update-models.sh"
