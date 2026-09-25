import json
import os
from pathlib import Path
import subprocess
import urllib.request
import urllib.parse

def get_github_token() -> str | None:
    # 1. Environment
    if os.environ.get("GITHUB_TOKEN"):
        return os.environ["GITHUB_TOKEN"]
    # 2. Git credential manager
    try:
        proc = subprocess.run(
            ["git", "credential", "fill"],
            input="protocol=https\nhost=github.com\n\n",
            capture_output=True,
            text=True,
            check=True,
            timeout=5,
        )
        for line in proc.stdout.splitlines():
            if line.startswith("password="):
                return line.split("=", 1)[1].strip()
    except Exception as e:
        print(f"Error fetching credential: {e}")
    return None

def main():
    token = get_github_token()
    if not token:
        print("ERROR: GitHub token not found.")
        return 1

    repo = "ankitshx/TELEVAULT"
    tag = "v2.1.0"
    headers = {
        "Authorization": f"Bearer {token}",
        "Accept": "application/vnd.github+json",
        "User-Agent": "TeleVault-Release-Uploader",
        "X-GitHub-Api-Version": "2022-11-28",
    }

    # 1. Check if release already exists
    req = urllib.request.Request(
        f"https://api.github.com/repos/{repo}/releases/tags/{tag}",
        headers=headers,
    )
    release_data = None
    try:
        with urllib.request.urlopen(req) as resp:
            release_data = json.loads(resp.read().decode())
            print(f"Existing release found: id={release_data['id']}")
    except urllib.error.HTTPError as e:
        if e.code == 404:
            print("Release not found. Creating new release...")
        else:
            print(f"HTTPError checking release: {e.code} {e.read().decode()}")

    if not release_data:
        # Create release
        create_payload = json.dumps({
            "tag_name": tag,
            "target_commitish": "main",
            "name": "TeleVault v2.1.0 — Desktop Application Release",
            "body": (
                "## 🚀 TeleVault Desktop Application (Windows 11 / 10)\n\n"
                "### Features Included:\n"
                "- **Native Telegram MTProto Login:** Connect seamlessly directly inside the GUI using your phone and OTP code (with optional 2FA password support).\n"
                "- **2GB+ Dynamic Multi-Part Chunking:** Upload large files of any size with automatic slicing, master post tracking, and chunk-hash verification.\n"
                "- **Dual-Write Redundancy:** Automatic simultaneous backup to Primary and Mirror vault channels.\n"
                "- **Clean Fluent Desktop UI:** Fast drag-and-drop file protection, streaming previewer, 1-click restore, and self-healing.\n\n"
                "### Direct Download:\n"
                "- **Standalone Executable:** [`televault.exe`](https://github.com/ankitshx/TELEVAULT/releases/download/v2.1.0/televault.exe)\n"
                "- **Zip Bundle:** [`TeleVault-v2.1.0-Windows-x64.zip`](https://github.com/ankitshx/TELEVAULT/releases/download/v2.1.0/TeleVault-v2.1.0-Windows-x64.zip)\n"
            ),
            "draft": False,
            "prerelease": False,
        }).encode("utf-8")

        req = urllib.request.Request(
            f"https://api.github.com/repos/{repo}/releases",
            data=create_payload,
            headers={**headers, "Content-Type": "application/json"},
        )
        try:
            with urllib.request.urlopen(req) as resp:
                release_data = json.loads(resp.read().decode())
                print(f"Created release id={release_data['id']}")
        except urllib.error.HTTPError as e:
            print(f"Failed to create release: {e.code} {e.read().decode()}")
            return 1

    release_id = release_data["id"]
    upload_url_template = release_data["upload_url"] # e.g. "https://uploads.github.com/repos/ankitshx/TELEVAULT/releases/123/assets{?name,label}"
    base_upload_url = upload_url_template.split("{")[0]

    # Delete any existing assets with matching names to avoid conflict
    existing_assets = {a["name"]: a["id"] for a in release_data.get("assets", [])}

    files_to_upload = [
        Path("dist/televault.exe"),
        Path("dist/TeleVault-v2.1.0-Windows-x64.zip"),
    ]

    for fpath in files_to_upload:
        if not fpath.exists():
            print(f"Skipping {fpath}, file does not exist.")
            continue

        if fpath.name in existing_assets:
            asset_id = existing_assets[fpath.name]
            print(f"Deleting older asset {fpath.name} (id={asset_id})...")
            del_req = urllib.request.Request(
                f"https://api.github.com/repos/{repo}/releases/assets/{asset_id}",
                headers=headers,
                method="DELETE",
            )
            try:
                with urllib.request.urlopen(del_req) as resp:
                    pass
            except Exception as e:
                print(f"Could not delete old asset: {e}")

        print(f"Uploading {fpath.name} ({fpath.stat().st_size / (1024*1024):.1f} MB)...")
        content_type = "application/vnd.microsoft.portable-executable" if fpath.suffix == ".exe" else "application/zip"
        with open(fpath, "rb") as f:
            data = f.read()

        params = urllib.parse.urlencode({"name": fpath.name})
        upload_req = urllib.request.Request(
            f"{base_upload_url}?{params}",
            data=data,
            headers={
                **headers,
                "Content-Type": content_type,
            },
        )
        try:
            with urllib.request.urlopen(upload_req) as resp:
                asset_res = json.loads(resp.read().decode())
                print(f"Uploaded {fpath.name} successfully! Download URL:\n{asset_res.get('browser_download_url')}")
        except urllib.error.HTTPError as e:
            print(f"Upload failed for {fpath.name}: {e.code} {e.read().decode()}")

    print("\nSUCCESS! Release is live at: https://github.com/ankitshx/TELEVAULT/releases/latest")
    return 0

if __name__ == "__main__":
    import sys
    sys.exit(main())
