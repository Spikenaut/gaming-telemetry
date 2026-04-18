# /// script
# dependencies = [
#   "huggingface_hub",
# ]
# ///

import os
from huggingface_hub import HfApi

def upload_dataset(repo_id: str, pattern: str, path_in_repo: str, hf_token: str = None, local_dir: str = "."):
    """
    Upload files matching a pattern to a Hugging Face Dataset repository.
    """
    print(f"Preparing to upload files matching '{pattern}' to dataset '{repo_id}'...")
    
    # Initialize the API
    # Using the token from args if provided, otherwise looks for env var HF_TOKEN
    api = HfApi(token=hf_token)
    
    # Verify or create the dataset repo
    try:
        api.create_repo(repo_id=repo_id, repo_type="dataset", exist_ok=True)
        print(f"Ensured repository '{repo_id}' exists.")
    except Exception as e:
        print(f"Error checking/creating repo: {e}")
        return

    print(f"Uploading files to {path_in_repo}/...")
    
    # OPTIMIZATION: upload_folder is significantly faster than looping api.upload_file().
    # It handles parallel uploads and bundles everything into a single commit history.
    try:
        api.upload_folder(
            folder_path=local_dir,
            path_in_repo=path_in_repo,
            repo_id=repo_id,
            repo_type="dataset",
            allow_patterns=pattern,
            commit_message=f"Upload dataset files matching {pattern} to {path_in_repo}"
        )
        print("\nUpload complete! Your dataset files are now available on Hugging Face.")
        
        # Finally, upload the dataset card as README.md to the root
        readme_path = "hf_dataset_card.md"
        if os.path.exists(readme_path):
            print(f"Uploading dataset card: {readme_path} -> README.md")
            api.upload_file(
                path_or_fileobj=readme_path,
                path_in_repo="README.md",
                repo_id=repo_id,
                repo_type="dataset",
                commit_message="Add Dataset Card description (README.md)"
            )
        
        print(f"View it here: https://huggingface.co/datasets/{repo_id}")
    except Exception as e:
        print(f"Failed to upload folder: {e}")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Upload Parquet files to a Hugging Face Dataset repo")
    parser.add_argument("--repo-id", type=str, default="rmems/Metis-SMoE-Latent-Telemetry", 
                        help="The Hugging Face repository ID (e.g., username/repo_name)")
    parser.add_argument("--path-in-repo", type=str, default="origin_hardware_baselines/resident_evil_4", 
                        help="The folder path inside the Hugging Face repo to upload to")
    parser.add_argument("--pattern", type=str, default="system_telemetry_v1_batch_*.parquet", 
                        help="Glob pattern for finding parquet files")
    parser.add_argument("--dir", type=str, default=".", 
                        help="Local directory containing the files to upload (defaults to current dir)")
    parser.add_argument("--token", type=str, default=None, 
                        help="Hugging Face API token with write access (or run 'huggingface-cli login')")
    
    args = parser.parse_args()
    
    # It prefers the token argument, then looks for HF_TOKEN env var.
    token = args.token or os.environ.get("HF_TOKEN")
    
    upload_dataset(args.repo_id, args.pattern, args.path_in_repo, token, args.dir)