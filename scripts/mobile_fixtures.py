"""Create a fresh disposable library for native rename/Undo acceptance tests.

Files are inert rename fixtures, NOT playable media. Never run on a real library.
"""
import argparse
import hashlib
import json
from pathlib import Path


def create(root):
    root = Path(root).resolve()
    if root.exists() and any(root.iterdir()):
        raise ValueError('Fixture directory must be new or empty; nothing was changed')
    root.mkdir(parents=True, exist_ok=True)
    entries = {
        'Manual/s1/01.mkv': b'Media Manager fixture A: not a playable video.\n',
        'Manual/s1/02.mkv': b'Media Manager fixture B: not a playable video.\n',
        'Manual/s1/01.en.forced.srt': b'1\n00:00:00,000 --> 00:00:01,000\nTest subtitle\n',
        'Manual/s1/01.ja.mka': b'Media Manager external audio rename fixture.\n',
        'Manual/s1/01.nfo': b'<episodedetails><title>Pilot</title></episodedetails>\n',
        'Anime/One Piece - 001.mkv': b'Anime absolute numbering fixture 001.\n',
        'Anime/One Piece - 003.mkv': b'Anime absolute numbering fixture 003.\n',
        'Duplicates/01.mkv': b'Exact duplicate media fixture: not a video.\n',
        'Duplicates/02.mkv': b'Exact duplicate media fixture: not a video.\n',
        'Duplicates/03.mkv': b'Different same-size fixture: not a video!!!\n',
        'Conflict/s1/01.mkv': b'Keep this original fixture.\n',
        'Conflict/s1/S01E01 - Pilot.mkv': b'Existing destination must never be overwritten.\n',
        'Manual/poster.jpg': b'Existing poster sentinel; keep these exact bytes.\n',
    }
    manifest = {}
    for relative, data in entries.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open('xb') as output:
            output.write(data)
        manifest[relative] = hashlib.sha256(data).hexdigest()
    (root / 'fixture-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    return manifest


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    print(f'Created {len(create(args.directory))} disposable files in {args.directory.resolve()}')
