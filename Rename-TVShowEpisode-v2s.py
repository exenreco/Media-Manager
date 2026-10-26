#!/usr/bin/env python3
import os
import re
import sys
import urllib.parse
from pathlib import Path

import requests

def sanitize_title(title: str) -> str:
    # remove characters invalid in filenames on Windows: \ / : * ? " < > |
    return re.sub(r'[\\/:*?"<>|]', '', title).strip()

def fetch_episodes(show_name: str, season: int):
    base_url = 'https://api.tvmaze.com'
    query = urllib.parse.quote(show_name)
    # find the show
    resp = requests.get(f'{base_url}/singlesearch/shows?q={query}')
    resp.raise_for_status()
    show = resp.json()
    # fetch all episodes, then filter by season
    resp = requests.get(f'{base_url}/shows/{show["id"]}/episodes')
    resp.raise_for_status()
    episodes = [
        ep for ep in resp.json()
        if ep.get('season') == season
    ]
    # sort by episode number
    return show, sorted(episodes, key=lambda e: e.get('number', 0))

def main():
    # Prompt user
    show_name = input("Enter TV show name (e.g., Breaking Bad): ").strip()
    try:
        season = int(input("Enter season number (e.g., 1): ").strip())
    except ValueError:
        print("Season must be an integer.")
        sys.exit(1)

    try:
        year = int(input("Enter year of the series (e.g., 2008): ").strip())
    except ValueError:
        print("Year must be an integer.")
        sys.exit(1)

    print(f"\n🔎 Searching for '{show_name}' (Season {season}, Year {year})...\n")
    try:
        show, episodes = fetch_episodes(show_name, season)
    except requests.HTTPError as e:
        print(f"❌ API error: {e}")
        sys.exit(1)

    if not episodes:
        print(f"❌ No episodes found for Season {season}.")
        sys.exit(1)

    # Validate year
    premiered = show.get("premiered")
    if not premiered:
        print(f"⚠️ Could not find premiere year for '{show_name}'.")
    else:
        premiere_year = int(premiered.split("-")[0])
        if premiere_year != year:
            print(f"❌ Show year mismatch: '{show_name}' premiered in {premiere_year}, not {year}.")
            sys.exit(1)

    # Gather files
    script_name = Path(__file__).name
    exts = {'.mp4', '.mkv', '.avi', '.txt'}
    folder = Path.cwd()
    files = sorted(
        [f for f in folder.iterdir() if f.is_file()
         and f.name != script_name
         and f.suffix.lower() in exts],
        key=lambda f: f.name
    )

    if not files:
        print("⚠️ No media/text files found in this folder.")
        sys.exit(1)

    print(f"\n📂 Found {len(files)} file(s) — renaming now...\n")

    # Rename loop
    count = min(len(files), len(episodes))
    for i in range(count):
        file = files[i]
        ep   = episodes[i]
        s    = f"{season:02d}"
        e    = f"{ep['number']:02d}"
        title = sanitize_title(ep.get('name', ''))
        new_name = f"S{s}E{e} - {title}{file.suffix}"
        try:
            file.rename(folder / new_name)
            print(f"✅ {file.name} → {new_name}")
        except Exception as ex:
            print(f"❌ Failed to rename {file.name}: {ex}")

    print(f"\n🎉 Done! Renamed {count} file(s) for '{show_name}' Season {season}.\n")

if __name__ == "__main__":
    main()