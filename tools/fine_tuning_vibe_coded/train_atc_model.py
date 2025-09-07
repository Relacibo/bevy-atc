#!/usr/bin/env python3
"""
ATC Whisper Fine-tuning Tool for Bevy ATC Project
Trains a custom Whisper model on aviation communication data.
"""

# IMPORTANT: Block TorchCodec before any other imports to prevent FFmpeg/ROCm conflicts
import os
import sys
import warnings

# Prevent TorchCodec from loading
os.environ['DISABLE_TORCHCODEC'] = '1'
os.environ['HF_DATASETS_DISABLE_AUDIO_AUTO_DECODE'] = '1'
os.environ['DATASETS_DISABLE_AUDIO_AUTO_DECODE'] = '1'
os.environ['TOKENIZERS_PARALLELISM'] = 'false'  # Prevent tokenizer fork warnings
os.environ['PYTORCH_HIP_ALLOC_CONF'] = 'expandable_segments:True'  # Better HIP memory management

# Block torchcodec imports completely
class FakeTorchCodec:
    def __getattr__(self, name):
        return None  # Return None instead of raising ImportError for compatibility
    
    def __bool__(self):
        return False  # Make it falsy for if torchcodec checks

# Only block actual imports, not spec checks
import importlib.util
original_find_spec = importlib.util.find_spec

def patched_find_spec(name, package=None):
    if name and 'torchcodec' in name:
        return None  # Pretend torchcodec doesn't exist
    return original_find_spec(name, package)

importlib.util.find_spec = patched_find_spec

sys.modules['torchcodec'] = FakeTorchCodec()
sys.modules['torchcodec.decoders'] = FakeTorchCodec()

# Suppress related warnings
warnings.filterwarnings('ignore', message='.*torchcodec.*')
warnings.filterwarnings('ignore', message='.*FFmpeg.*')

import argparse
import pandas as pd
import torch
import numpy as np
from pathlib import Path
from datasets import Dataset, Audio
from transformers import (
    WhisperProcessor,
    WhisperForConditionalGeneration,
    Trainer,
    Seq2SeqTrainingArguments,
    EarlyStoppingCallback
)
from dataclasses import dataclass
from typing import Any, Dict, List, Union
import json
import logging

# Setup logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

@dataclass
class DataCollatorSpeechSeq2SeqWithPadding:
    processor: Any
    decoder_start_token_id: int

    def __call__(self, features: List[Dict[str, Union[List[int], torch.Tensor]]]) -> Dict[str, torch.Tensor]:
        # Separate audio and text features
        model_input_name = self.processor.model_input_names[0]
        input_features = [{model_input_name: feature[model_input_name]} for feature in features]
        
        # Extract labels for efficient batch processing
        label_input_ids = [feature["labels"] for feature in features]
        
        # Batch process audio features
        batch = self.processor.feature_extractor.pad(input_features, return_tensors="pt")
        
        # Pad the already tokenized IDs directly (most efficient approach)
        max_label_length = max(len(label) for label in label_input_ids)
        padded_labels = []
        
        for label in label_input_ids:
            # Pad with -100 (ignore index for loss calculation)
            padded_label = label + [-100] * (max_label_length - len(label))
            padded_labels.append(padded_label)
        
        labels = torch.tensor(padded_labels, dtype=torch.long)
        
        # Remove decoder start token if present (Whisper specific)
        if len(labels) > 0 and labels.size(1) > 0:
            if (labels[:, 0] == self.decoder_start_token_id).all().cpu().item():
                labels = labels[:, 1:]

        batch["labels"] = labels
        return batch

def prepare_dataset(batch, processor):
    """Prepare dataset for training - uses librosa for robust audio loading"""
    import librosa
    import os
    
    try:
        # Handle audio loading based on available data
        audio_path = None
        audio_array = None
        
        # Try different possible audio column names
        possible_audio_cols = ["audio", "audio_path", "path", "file", "filename"]
        for col in possible_audio_cols:
            if col in batch and batch[col] is not None:
                potential_path = batch[col]
                
                # Handle different path formats
                if isinstance(potential_path, str):
                    # Direct path string
                    if os.path.exists(potential_path):
                        audio_path = potential_path
                        break
                elif isinstance(potential_path, dict):
                    # Audio object with path
                    if "path" in potential_path:
                        path = potential_path["path"]
                        if os.path.exists(path):
                            audio_path = path
                            break
                    elif "array" in potential_path:
                        # Pre-loaded audio array
                        audio_array = np.array(potential_path["array"], dtype=np.float32)
                        break
        
        # Load audio using librosa if we have a path
        if audio_path and audio_array is None:
            try:
                # Handle relative paths by making them absolute
                if not os.path.isabs(audio_path):
                    # Try to make path relative to dataset directory
                    base_dir = "/home/reinhard/git/bevy-atc/crates/atc_recognition_rs/resources/datasets/ATC-ASR-Dataset"
                    audio_path = os.path.join(base_dir, audio_path)
                
                if os.path.exists(audio_path):
                    audio_array, sampling_rate = librosa.load(
                        audio_path, 
                        sr=16000,  # Whisper expects 16kHz
                        mono=True  # Convert to mono
                    )
                    logger.debug(f"✅ Loaded audio with librosa: {audio_path} ({len(audio_array)} samples)")
                else:
                    logger.warning(f"Audio file not found: {audio_path}")
                    audio_array = np.zeros(16000, dtype=np.float32)
            except Exception as load_error:
                logger.warning(f"Failed to load audio file {audio_path}: {load_error}")
                # Create silence as fallback (1 second at 16kHz)
                audio_array = np.zeros(16000, dtype=np.float32)
        
        # If we still don't have audio, create silence
        if audio_array is None:
            logger.debug("No audio data available, using silence")
            audio_array = np.zeros(16000, dtype=np.float32)
        
        # Ensure audio is not empty and has correct format
        if len(audio_array) == 0:
            logger.warning("Empty audio array, using silence")
            audio_array = np.zeros(16000, dtype=np.float32)
        
        # Ensure correct dtype
        if not isinstance(audio_array, np.ndarray):
            audio_array = np.array(audio_array, dtype=np.float32)
        else:
            audio_array = audio_array.astype(np.float32)
        
        # Compute log-Mel input features
        try:
            batch["input_features"] = processor.feature_extractor(
                audio_array, 
                sampling_rate=16000,  # Always use 16kHz
                return_tensors="np"
            ).input_features[0]
            
        except Exception as feature_error:
            logger.warning(f"Feature extraction failed: {feature_error}")
            # Create empty features as fallback (80 mel bins, 3000 time steps for Whisper)
            batch["input_features"] = np.zeros((80, 3000), dtype=np.float32)
        
    except Exception as e:
        logger.warning(f"Audio processing failed completely: {e}")
        # Create empty features as fallback
        batch["input_features"] = np.zeros((80, 3000), dtype=np.float32)
    
    # Use the faster __call__ method for tokenization
    # This is more efficient than calling .tokenizer() and then .pad()
    try:
        tokenized = processor.tokenizer(
            batch["transcription"],
            truncation=True,
            max_length=225,
            padding=False,  # We'll pad in the data collator
            return_tensors=None  # Return as lists for now
        )
        
        batch["labels"] = tokenized.input_ids
    except Exception as e:
        logger.warning(f"Tokenization failed: {e}")
        # Create empty labels as fallback
        batch["labels"] = [processor.tokenizer.eos_token_id]
    
    return batch

def load_atc_dataset(dataset_path: Path):
    """Load ATC dataset from various formats - bypasses automatic audio loading"""
    if dataset_path.name == "ATC-ASR-Dataset":
        # Load from Hugging Face Parquet format
        logger.info("📊 Loading ATC-ASR-Dataset (Parquet format)...")
        
        try:
            # Load directly from Parquet files to avoid TorchCodec
            import pandas as pd
            
            # Load train splits
            train_files = [
                dataset_path / "data" / "train-00000-of-00002.parquet",
                dataset_path / "data" / "train-00001-of-00002.parquet"
            ]
            train_dfs = []
            for file in train_files:
                if file.exists():
                    train_dfs.append(pd.read_parquet(file))
            
            train_df = pd.concat(train_dfs, ignore_index=True) if train_dfs else pd.DataFrame()
            
            # Load validation split
            val_file = dataset_path / "data" / "validation-00000-of-00001.parquet"
            val_df = pd.read_parquet(val_file) if val_file.exists() else pd.DataFrame()
            
            # Load test split
            test_file = dataset_path / "data" / "test-00000-of-00001.parquet"
            test_df = pd.read_parquet(test_file) if test_file.exists() else pd.DataFrame()
            
            logger.info(f"✅ Loaded ATC-ASR-Dataset from Parquet:")
            logger.info(f"  - Train: {len(train_df)} samples")
            logger.info(f"  - Validation: {len(val_df)} samples") 
            logger.info(f"  - Test: {len(test_df)} samples")
            
            if len(train_df) > 0:
                logger.info(f"  - Columns: {list(train_df.columns)}")
                
                # Show sample data for debugging
                sample = train_df.iloc[0]
                logger.info(f"  - Sample data:")
                for col in train_df.columns:
                    value = sample[col]
                    if isinstance(value, str) and len(value) > 100:
                        value = value[:100] + "..."
                    logger.info(f"    {col}: {value}")
            
            # Convert to datasets format, avoiding audio auto-loading
            from datasets import Dataset
            
            # Rename 'text' to 'transcription' if needed
            if "text" in train_df.columns:
                train_df = train_df.rename(columns={"text": "transcription"})
                val_df = val_df.rename(columns={"text": "transcription"}) if len(val_df) > 0 else val_df
            
            # Convert to Dataset objects
            train_dataset = Dataset.from_pandas(train_df) if len(train_df) > 0 else None
            val_dataset = Dataset.from_pandas(val_df) if len(val_df) > 0 else None
            test_dataset = Dataset.from_pandas(test_df) if len(test_df) > 0 else None
            
            return {
                "train": train_dataset, 
                "validation": val_dataset, 
                "test": test_dataset
            }
            
        except Exception as e:
            logger.error(f"Failed to load ATC-ASR-Dataset: {e}")
            raise
    
    elif dataset_path.suffix == '.ron':
        # Load from RON index (like your test recordings)
        logger.info("Loading from RON format - converting to training data...")
        raise NotImplementedError("RON format support needs implementation")
        
    elif dataset_path.suffix == '.csv':
        # Load from CSV
        df = pd.read_csv(dataset_path)
        logger.info(f"Loaded {len(df)} samples from CSV")
        
        # Validate columns
        required_columns = ['audio_path', 'transcription']
        if not all(col in df.columns for col in required_columns):
            raise ValueError(f"CSV must contain columns: {required_columns}")
        
        # Convert relative paths to absolute
        base_dir = dataset_path.parent
        df['audio_path'] = df['audio_path'].apply(lambda x: str(base_dir / x))
        
        # Convert to dataset
        dataset = Dataset.from_pandas(df)
        dataset = dataset.cast_column("audio_path", Audio(sampling_rate=16000))
        dataset = dataset.rename_column("audio_path", "audio")
        
        return {"train": dataset, "validation": None, "test": None}
        
    else:
        # Load from directory structure
        audio_dir = dataset_path / "audio"
        transcript_file = dataset_path / "transcripts.csv"
        
        if transcript_file.exists():
            df = pd.read_csv(transcript_file)
            df['audio_path'] = df['filename'].apply(lambda x: str(audio_dir / x))
            dataset = Dataset.from_pandas(df)
            dataset = dataset.cast_column("audio_path", Audio(sampling_rate=16000))
            dataset = dataset.rename_column("audio_path", "audio")
            return {"train": dataset, "validation": None, "test": None}
        else:
            raise ValueError(f"No recognized dataset format found in {dataset_path}")
    
    return dataset

def create_csv_from_ron_index(ron_path: Path, output_csv: Path):
    """Convert RON index to CSV format for training"""
    # This is a helper function to convert your test-recordings index
    logger.info(f"Converting {ron_path} to {output_csv}")
    
    # You would implement RON parsing here
    # For now, create a template
    data = {
        'audio_path': ['../test-recordings/delta.wav', '../test-recordings/easyjet.wav'],
        'transcription': [
            'delta four one three zero fly heading three two five',
            'easyjet two niner zero two climb and maintain flight level two hundred fifty and turn left heading three two zero'
        ]
    }
    
    df = pd.DataFrame(data)
    df.to_csv(output_csv, index=False)
    logger.info(f"Created template CSV at {output_csv}")
    return output_csv

def main():
    parser = argparse.ArgumentParser(description="Fine-tune Whisper model for ATC")
    parser.add_argument("--dataset", type=str, 
                       default="../resources/datasets/ATC-ASR-Dataset",
                       help="Path to dataset (ATC-ASR-Dataset directory, CSV file, etc.)")
    parser.add_argument("--base_model", type=str, default="openai/whisper-small.en",  # .en = English-only
                       help="Base Whisper model to fine-tune")
    parser.add_argument("--output_dir", type=str, default="./output/whisper-atc",
                       help="Output directory for trained model")
    parser.add_argument("--epochs", type=int, default=2,  # Reduce to 1 epoch
                       help="Number of training epochs")
    parser.add_argument("--batch_size", type=int, default=2,
                       help="Training batch size (reduced for GPU memory)")
    parser.add_argument("--learning_rate", type=float, default=1e-5,
                       help="Learning rate")
    parser.add_argument("--max_steps", type=int, default=None,
                       help="Maximum training steps (overrides epochs)")
    parser.add_argument("--use_validation", action="store_true", default=True,
                       help="Use validation split from ATC-ASR-Dataset")
    
    args = parser.parse_args()
    
    # Setup paths
    dataset_path = Path(args.dataset)
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    
    logger.info(f"🎯 Starting ATC Whisper fine-tuning")
    logger.info(f"📁 Dataset: {dataset_path}")
    logger.info(f"🤖 Base model: {args.base_model}")
    logger.info(f"📦 Output: {output_dir}")
    
    # Check GPU availability and clear memory
    import torch
    if torch.cuda.is_available():
        device_count = torch.cuda.device_count()
        logger.info(f"🔴 ROCm/CUDA available with {device_count} device(s)")
        
        # Clear GPU cache to start fresh
        torch.cuda.empty_cache()
        
        for i in range(device_count):
            try:
                device_name = torch.cuda.get_device_name(i)
                memory_allocated = torch.cuda.memory_allocated(i) / 1024**3
                memory_reserved = torch.cuda.memory_reserved(i) / 1024**3
                memory_total = torch.cuda.get_device_properties(i).total_memory / 1024**3
                logger.info(f"  GPU {i}: {device_name}")
                logger.info(f"    Total: {memory_total:.1f}GB, Allocated: {memory_allocated:.1f}GB, Reserved: {memory_reserved:.1f}GB")
            except:
                logger.info(f"  GPU {i}: Unknown device")
    else:
        logger.info("🖥️  Running on CPU only")
    
    # Check if dataset exists
    if not dataset_path.exists():
        logger.error(f"Dataset not found: {dataset_path}")
        logger.info("Please ensure ATC-ASR-Dataset is in resources/datasets/")
        return
    
    # Load dataset
    try:
        dataset_dict = load_atc_dataset(dataset_path)
        
        # Extract datasets based on type
        if isinstance(dataset_dict, dict):
            train_dataset = dataset_dict["train"]
            eval_dataset = dataset_dict["validation"] if args.use_validation else None
        else:
            # Single dataset, split it
            train_test = dataset_dict.train_test_split(test_size=0.1, seed=42)
            train_dataset = train_test["train"] 
            eval_dataset = train_test["test"] if args.use_validation else None
            
    except Exception as e:
        logger.error(f"Failed to load dataset: {e}")
        return
    
    logger.info(f"📊 Dataset loaded:")
    logger.info(f"  - Training samples: {len(train_dataset)}")
    if eval_dataset:
        logger.info(f"  - Validation samples: {len(eval_dataset)}")
    
    # Load model and processor
    logger.info(f"🔄 Loading {args.base_model}...")
    processor = WhisperProcessor.from_pretrained(args.base_model)
    model = WhisperForConditionalGeneration.from_pretrained(args.base_model)
    
    # Prepare datasets
    logger.info("🔄 Preprocessing datasets...")
    train_dataset = train_dataset.map(
        lambda batch: prepare_dataset(batch, processor),
        remove_columns=train_dataset.column_names,
        num_proc=1,  # Use single process to avoid TorchCodec/FFmpeg issues
        desc="Preprocessing training data"
    )
    
    if eval_dataset:
        eval_dataset = eval_dataset.map(
            lambda batch: prepare_dataset(batch, processor),
            remove_columns=eval_dataset.column_names,
            num_proc=1,  # Use single process to avoid TorchCodec/FFmpeg issues
            desc="Preprocessing validation data"
        )
    
    # Setup data collator
    data_collator = DataCollatorSpeechSeq2SeqWithPadding(
        processor=processor,
        decoder_start_token_id=model.generation_config.decoder_start_token_id,
    )
    
    # Calculate training steps for large dataset
    steps_per_epoch = len(train_dataset) // args.batch_size
    total_steps = args.max_steps if args.max_steps else args.epochs * steps_per_epoch
    
    logger.info(f"📈 Training configuration:")
    logger.info(f"  - Steps per epoch: {steps_per_epoch}")
    logger.info(f"  - Total training steps: {total_steps}")
    logger.info(f"  - Estimated training time: {total_steps * 0.5 / 60:.1f} minutes")
    
    # Training arguments optimized for 12GB GPU memory
    training_args = Seq2SeqTrainingArguments(
        output_dir=str(output_dir),
        per_device_train_batch_size=2,  # Reduce to 1 for memory safety
        gradient_accumulation_steps=8,  # Increase accumulation (effective batch = 4)
        learning_rate=args.learning_rate,
        warmup_steps=min(200, total_steps // 20),
        max_steps=total_steps,
        gradient_checkpointing=False,
        fp16=False,
        tf32=False,
        eval_strategy="steps" if eval_dataset else "no",
        per_device_eval_batch_size=2,  # Reduce eval batch size too
        predict_with_generate=True,
        generation_max_length=225,
        save_steps=max(200, total_steps // 10),
        eval_steps=max(200, total_steps // 10) if eval_dataset else None,
        logging_steps=50,
        report_to=["tensorboard"],
        load_best_model_at_end=True if eval_dataset else False,
        metric_for_best_model="eval_loss" if eval_dataset else None,
        greater_is_better=False,
        push_to_hub=False,
        dataloader_num_workers=2,  # Disable multiprocessing
        remove_unused_columns=False,
        save_total_limit=2,
        dataloader_pin_memory=True,  # Enable for faster transfer
        dataloader_persistent_workers=True,
        max_grad_norm=1.0,
    )
    
    # Setup trainer
    callbacks = []
    if eval_dataset:
        callbacks.append(EarlyStoppingCallback(early_stopping_patience=5))
    
    trainer = Trainer(
        model=model,
        args=training_args,
        train_dataset=train_dataset,
        eval_dataset=eval_dataset,
        data_collator=data_collator,
        processing_class=processor.feature_extractor,  # Updated parameter name
        callbacks=callbacks,
    )
    
    # Train!
    logger.info("🚀 Starting training...")
    logger.info(f"💡 Training {len(train_dataset)} samples with large ATC dataset")
    trainer.train()
    
    # Save model
    logger.info("💾 Saving trained model...")
    trainer.save_model(str(output_dir / "final"))
    processor.save_pretrained(str(output_dir / "final"))
    
    # Save training info
    training_info = {
        "base_model": args.base_model,
        "dataset_path": str(dataset_path),
        "dataset_type": "ATC-ASR-Dataset" if "ATC-ASR-Dataset" in str(dataset_path) else "Custom",
        "training_samples": len(train_dataset),
        "validation_samples": len(eval_dataset) if eval_dataset else 0,
        "epochs": args.epochs,
        "batch_size": args.batch_size,
        "effective_batch_size": args.batch_size * 2,  # Including gradient accumulation
        "learning_rate": args.learning_rate,
        "total_steps": total_steps,
    }
    
    with open(output_dir / "training_info.json", 'w') as f:
        json.dump(training_info, f, indent=2)
    
    logger.info(f"✅ Training complete! Model saved to {output_dir / 'final'}")
    logger.info(f"📊 Trained on {len(train_dataset)} ATC samples")
    logger.info("🔄 Next step: Convert to whisper.cpp format using convert_to_ggml.py")

if __name__ == "__main__":
    main()
