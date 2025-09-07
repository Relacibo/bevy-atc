#!/bin/bash
set -e

echo "🔄 Updating Bevy ATC Models"
echo "==========================="

MODELS_DIR="crates/atc_recognition_rs/resources/models/whisper-small.en-atc-experiment"

if [ ! -d "$MODELS_DIR" ]; then
    echo "❌ Error: Models directory not found. Run setup.sh first."
    exit 1
fi

echo "📥 Updating models submodule..."
cd "$MODELS_DIR"

# Update submodule
git pull origin main

# Download updated model files
echo "📥 Downloading latest model files..."
git lfs pull --include="whisper-atc-q8_0.bin"

cd - > /dev/null

# Update submodule reference in main repo
echo "📝 Updating submodule reference..."
git add "$MODELS_DIR"

if git diff --cached --quiet; then
    echo "✅ No updates available"
else
    git commit -m "Update whisper model submodule"
    echo "✅ Models updated and committed!"
fi

echo ""
echo "🎉 Model update complete!"
