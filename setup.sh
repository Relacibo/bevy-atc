#!/bin/bash
set -e

echo "🚁 Bevy ATC Setup Script"
echo "========================"

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "crates" ]; then
    echo "❌ Error: Please run this script from the bevy-atc root directory"
    exit 1
fi

echo "📦 Initializing submodules..."
git submodule update --init --recursive

echo "🤖 Setting up ML models..."
MODELS_DIR="crates/atc_recognition_rs/resources/models/whisper-small.en-atc-experiment"

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
    
    cd - > /dev/null
else
    echo "⚠️  Warning: Models directory not found. Submodule may not be initialized properly."
fi

echo "🔨 Building project..."
cargo build

echo ""
echo "🎉 Setup complete!"
echo ""
echo "To run the game:"
echo "  cargo run"
echo ""
echo "To update models later:"
echo "  ./scripts/update-models.sh"
