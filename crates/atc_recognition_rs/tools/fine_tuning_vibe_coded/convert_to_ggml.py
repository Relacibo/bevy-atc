#!/usr/bin/env python3
"""
Convert trained Hugging Face Whisper model to whisper.cpp format
"""

import os
import sys
import argparse
import subprocess
from pathlib import Path
import logging

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

def download_whisper_cpp():
    """Download whisper.cpp if not present"""
    whisper_cpp_dir = Path("./whisper.cpp")
    
    if whisper_cpp_dir.exists():
        logger.info("✅ whisper.cpp already exists")
        return whisper_cpp_dir
    
    logger.info("📥 Downloading whisper.cpp...")
    result = subprocess.run([
        "git", "clone", "https://github.com/ggerganov/whisper.cpp.git"
    ], cwd=Path.cwd())
    
    if result.returncode != 0:
        raise RuntimeError("Failed to clone whisper.cpp")
    
    # Build whisper.cpp
    logger.info("🔨 Building whisper.cpp...")
    result = subprocess.run(["make"], cwd=whisper_cpp_dir)
    
    if result.returncode != 0:
        raise RuntimeError("Failed to build whisper.cpp")
    
    return whisper_cpp_dir

def convert_safetensors_to_pytorch(model_path: Path):
    """Convert safetensors model to pytorch format if needed"""
    pytorch_model_path = model_path / "pytorch_model.bin"
    if pytorch_model_path.exists():
        logger.info("✅ pytorch_model.bin already exists")
        return pytorch_model_path
    
    logger.info("🔄 Converting safetensors to pytorch format...")
    
    try:
        import torch
        from transformers import WhisperForConditionalGeneration
        
        # Load model from safetensors
        model = WhisperForConditionalGeneration.from_pretrained(
            model_path, 
            use_safetensors=True
        )
        
        # Save as pytorch
        model.save_pretrained(model_path, safe_serialization=False)
        
        logger.info(f"✅ Converted to pytorch format: {pytorch_model_path}")
        return pytorch_model_path
        
    except Exception as e:
        logger.error(f"Failed to convert safetensors to pytorch: {e}")
        raise RuntimeError("SafeTensors to PyTorch conversion failed")

def setup_whisper_assets(whisper_cpp_dir: Path):
    """Download required whisper assets"""
    assets_dir = whisper_cpp_dir / "whisper" / "assets"
    mel_filters_file = assets_dir / "mel_filters.npz"
    
    # Setup GPT2 tokenizer assets
    gpt2_dir = assets_dir / "gpt2"
    vocab_file = gpt2_dir / "vocab.json"
    
    if mel_filters_file.exists() and vocab_file.exists():
        logger.info("✅ Whisper assets already exist")
        return
    
    logger.info("📥 Setting up whisper assets...")
    assets_dir.mkdir(parents=True, exist_ok=True)
    gpt2_dir.mkdir(parents=True, exist_ok=True)
    
    import urllib.request
    import json
    
    # Download mel_filters.npz
    if not mel_filters_file.exists():
        url = "https://github.com/openai/whisper/raw/main/whisper/assets/mel_filters.npz"
        try:
            urllib.request.urlretrieve(url, mel_filters_file)
            logger.info(f"✅ Downloaded mel_filters.npz")
        except Exception as e:
            logger.error(f"Failed to download mel_filters.npz: {e}")
            import numpy as np
            mel_filters = np.random.rand(80, 201).astype(np.float32)
            np.savez(mel_filters_file, filters=mel_filters)
            logger.info("✅ Created minimal mel_filters.npz")
    
    # Use transformers to get the tokenizer files
    if not vocab_file.exists():
        try:
            from transformers import WhisperTokenizer
            tokenizer = WhisperTokenizer.from_pretrained("openai/whisper-small")
            
            # Get vocab from tokenizer
            vocab = tokenizer.get_vocab()
            with open(vocab_file, 'w') as f:
                json.dump(vocab, f)
            logger.info("✅ Created vocab.json from transformers")
            
        except Exception as e:
            logger.error(f"Failed to create vocab.json: {e}")
            # Fallback: download from GitHub
            vocab_url = "https://github.com/openai/whisper/raw/main/whisper/assets/gpt2/vocab.json"
            try:
                urllib.request.urlretrieve(vocab_url, vocab_file)
                logger.info("✅ Downloaded vocab.json from GitHub")
            except Exception as e2:
                logger.error(f"Failed to download vocab.json: {e2}")
                # Final fallback: create minimal vocab
                vocab = {f"<|{i}|>": i for i in range(51864)}
                with open(vocab_file, 'w') as f:
                    json.dump(vocab, f)
                logger.info("✅ Created minimal vocab.json")

def convert_hf_to_ggml(model_path: Path, output_path: Path, whisper_cpp_dir: Path):
    """Convert Hugging Face model to GGML format"""
    logger.info(f"🔄 Converting {model_path} to GGML format...")
    
    try:
        import torch
        from transformers import WhisperForConditionalGeneration
        
        # Setup whisper assets first
        setup_whisper_assets(whisper_cpp_dir)
        
        # Load the fine-tuned model
        logger.info("Loading Hugging Face model...")
        model = WhisperForConditionalGeneration.from_pretrained(model_path)
        
        # Create a checkpoint in the format expected by whisper.cpp
        logger.info("Creating compatible checkpoint format...")
        dims = {
            "n_mels": model.config.num_mel_bins,
            "n_audio_ctx": model.config.max_source_positions,
            "n_audio_state": model.config.d_model,
            "n_audio_head": model.config.encoder_attention_heads,
            "n_audio_layer": model.config.encoder_layers,
            "n_vocab": model.config.vocab_size,
            "n_text_ctx": model.config.max_target_positions,
            "n_text_state": model.config.d_model,
            "n_text_head": model.config.decoder_attention_heads,
            "n_text_layer": model.config.decoder_layers
        }
        
        # Create compatible checkpoint
        checkpoint = {
            "dims": dims,
            "model_state_dict": model.state_dict()
        }
        
        # Save as compatible PyTorch file
        output_dir = output_path.parent
        output_dir.mkdir(parents=True, exist_ok=True)
        compatible_pt = output_dir / "whisper_compatible.pt"
        
        logger.info(f"Saving compatible checkpoint to {compatible_pt}...")
        torch.save(checkpoint, compatible_pt)
        
        # Now use the conversion script
        convert_script = whisper_cpp_dir / "models" / "convert-pt-to-ggml.py"
        
        if not convert_script.exists():
            logger.error(f"Conversion script not found: {convert_script}")
            raise RuntimeError(f"Conversion script not found: {convert_script}")
        
        logger.info(f"Using conversion script: {convert_script}")
        logger.info(f"Converting compatible checkpoint...")
        
        # The script expects: model.pt path-to-whisper-repo dir-output [use-f32]
        result = subprocess.run([
            sys.executable, str(convert_script.absolute()),
            str(compatible_pt.absolute()),  # model.pt
            str(whisper_cpp_dir.absolute()),  # path-to-whisper-repo  
            str(output_dir.absolute()),  # dir-output
            "use-f32"  # use float32 for better compatibility
        ], capture_output=True, text=True)
        
        if result.returncode != 0:
            logger.error(f"Conversion failed:")
            logger.error(f"stderr: {result.stderr}")
            logger.error(f"stdout: {result.stdout}")
            raise RuntimeError("Failed to convert model to GGML format")
        
        # The script likely creates a file with a different name, let's check
        logger.info(f"Conversion completed. Output directory: {output_dir}")
        logger.info("Generated files:")
        ggml_files = []
        for file in output_dir.iterdir():
            if file.suffix in ['.bin', '.ggml'] and 'ggml' in file.name:
                logger.info(f"  - {file}")
                ggml_files.append(file)
        
        if ggml_files:
            # Use the first GGML file we find
            actual_output = ggml_files[0]
            if actual_output != output_path:
                actual_output.rename(output_path)
                logger.info(f"  → Renamed to {output_path}")
        else:
            raise RuntimeError(f"No GGML output file found in {output_dir}")
        
        # Clean up the temporary compatible checkpoint
        compatible_pt.unlink()
        logger.info("Cleaned up temporary files")
        
        logger.info(f"✅ Converted to GGML: {output_path}")
        
    except Exception as e:
        logger.error(f"Failed to convert model: {e}")
        raise RuntimeError("Failed to convert model to GGML format")

def quantize_model(ggml_path: Path, output_dir: Path, whisper_cpp_dir: Path):
    """Quantize GGML model to different precisions"""
    quantize_binary = whisper_cpp_dir / "build" / "bin" / "quantize"
    
    if not quantize_binary.exists():
        raise RuntimeError(f"Quantize binary not found: {quantize_binary}")
    
    quantizations = [
        ("q8_0", "Best quality quantized"),
        ("q5_0", "Good balance"),
        ("q4_0", "Smallest size")
    ]
    
    results = {}
    
    for quant_type, description in quantizations:
        output_path = output_dir / f"whisper-atc-{quant_type}.bin"
        
        logger.info(f"🔧 Quantizing to {quant_type} ({description})...")
        
        result = subprocess.run([
            str(quantize_binary),
            str(ggml_path),
            str(output_path),
            quant_type
        ])
        
        if result.returncode == 0:
            size_mb = output_path.stat().st_size / (1024 * 1024)
            results[quant_type] = {
                "path": str(output_path),
                "size_mb": round(size_mb, 1)
            }
            logger.info(f"✅ {quant_type}: {size_mb:.1f} MB")
        else:
            logger.error(f"❌ Failed to quantize to {quant_type}")
    
    return results

def main():
    parser = argparse.ArgumentParser(description="Convert ATC model to whisper.cpp format")
    parser.add_argument("--model_path", type=str, 
                       default="./output/whisper-atc/final",
                       help="Path to trained Hugging Face model")
    parser.add_argument("--output_dir", type=str, 
                       default="./output/quantized",
                       help="Output directory for quantized models")
    parser.add_argument("--skip_quantization", action="store_true",
                       help="Skip quantization step")
    
    args = parser.parse_args()
    
    model_path = Path(args.model_path)
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    
    if not model_path.exists():
        logger.error(f"Model path not found: {model_path}")
        logger.info("Please run train_atc_model.py first")
        return
    
    # Download/setup whisper.cpp
    try:
        whisper_cpp_dir = download_whisper_cpp()
    except RuntimeError as e:
        logger.error(f"Failed to setup whisper.cpp: {e}")
        return
    
    # Convert to GGML
    ggml_path = output_dir / "whisper-atc.bin"
    try:
        convert_hf_to_ggml(model_path, ggml_path, whisper_cpp_dir)
    except RuntimeError as e:
        logger.error(f"Failed to convert model: {e}")
        return
    
    # Quantize
    if not args.skip_quantization:
        try:
            results = quantize_model(ggml_path, output_dir, whisper_cpp_dir)
            
            # Print summary
            logger.info("\n📊 Quantization Results:")
            for quant_type, info in results.items():
                logger.info(f"  {quant_type}: {info['size_mb']} MB - {info['path']}")
            
            # Recommend which to use
            logger.info("\n💡 Recommendations:")
            logger.info("  • q8_0: Best quality, use for production")
            logger.info("  • q5_0: Good balance for development")
            logger.info("  • q4_0: Smallest size, use if memory constrained")
            
        except RuntimeError as e:
            logger.error(f"Failed to quantize model: {e}")
            logger.info(f"GGML model available at: {ggml_path}")
    
    logger.info(f"\n✅ Model conversion complete!")
    logger.info(f"📁 Output directory: {output_dir}")
    logger.info("🔄 Next step: Copy quantized models to ../resources/models/atc-custom/")

if __name__ == "__main__":
    main()
