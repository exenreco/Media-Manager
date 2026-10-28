"""Create signed desktop updater metadata from a fully published GitHub release.

The Ed25519 PKCS8 PEM key is provided via UPDATER_SIGNING_KEY, never a CLI argument.
"""
import argparse, base64, hashlib, json, os, pathlib, re, subprocess, tempfile, urllib.request

ROOT = 'https://github.com/exenreco/Media-Manager/releases/download/'

def manifest(release):
    tag = release['tag_name']
    if not re.fullmatch(r'v\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?', tag):
        raise ValueError('Invalid release version')
    if release.get('draft'):
        raise ValueError('Draft releases are not updater candidates')
    version = tag[1:]
    assets = {a['name']: a for a in release['assets']}
    packages = []
    targets = [('windows','x86_64','exe','windows-x86_64-setup.exe'),
               ('macos','aarch64','pkg','macos-aarch64.pkg'),
               ('macos','x86_64','pkg','macos-x86_64.pkg'),
               ('linux','x86_64','deb','linux-x86_64.deb'),
               ('linux','x86_64','flatpak','linux-x86_64.flatpak')]
    for platform, arch, kind, suffix in targets:
        name = f'Media-Manager-{version}-{suffix}'
        asset = assets.get(name)
        if asset is None:
            raise ValueError('Release is incomplete: ' + name)
        digest = asset.get('digest', '')
        if not re.fullmatch(r'sha256:[a-f0-9]{64}', digest):
            raise ValueError('Missing GitHub asset digest: ' + name)
        url = ROOT + tag + '/' + name
        if asset['browser_download_url'] != url or not 0 < asset['size'] <= 1024**3:
            raise ValueError('Invalid release asset: ' + name)
        packages.append(dict(platform=platform, arch=arch, kind=kind, name=name,
                             url=url, sha256=digest[7:], size=asset['size']))
    return dict(schema=1, version=version, packages=packages)

def sign(payload, key, pinned_public):
    with tempfile.TemporaryDirectory(prefix='media-manager-sign-') as temp:
        root=pathlib.Path(temp); private=root/'key.pem'; message=root/'manifest.json'; signature=root/'signature'
        private.write_text(key); private.chmod(0o600); message.write_bytes(payload)
        public=subprocess.check_output(['openssl','pkey','-in',str(private),'-pubout','-outform','DER'],stderr=subprocess.PIPE)
        if public != bytes.fromhex('302a300506032b6570032100')+pinned_public:
            raise ValueError('Signing key does not match the public key bundled with the app')
        subprocess.run(['openssl','pkeyutl','-sign','-rawin','-inkey',str(private),'-in',str(message),'-out',str(signature)],check=True,capture_output=True)
        return base64.b64encode(signature.read_bytes()) + b'\n'

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag',required=True);parser.add_argument('--out',type=pathlib.Path,default=pathlib.Path('dist/update-metadata'))
    args=parser.parse_args()
    request=urllib.request.Request('https://api.github.com/repos/exenreco/Media-Manager/releases/tags/'+args.tag,
        headers={'Authorization':'Bearer '+os.environ['GH_TOKEN'],'User-Agent':'Media-Manager-update-metadata','Accept':'application/vnd.github+json'})
    with urllib.request.urlopen(request,timeout=30) as response: release=json.load(response)
    if release['tag_name'] != args.tag:raise ValueError('Release tag mismatch')
    payload=(json.dumps(manifest(release),sort_keys=True,separators=(',',':'))+'\n').encode()
    public=base64.b64decode(pathlib.Path('crates/app/src/update-public-key.txt').read_text().strip(),validate=True)
    signature=sign(payload,os.environ['UPDATER_SIGNING_KEY'],public)
    args.out.mkdir(parents=True,exist_ok=True)
    for name,data in [('update-manifest.json',payload),('update-manifest.sig',signature)]:
        (args.out/name).write_bytes(data)
        (args.out/(name+'.sha256')).write_text(hashlib.sha256(data).hexdigest()+'  '+name+'\n')
    print('Signed desktop updater metadata for',args.tag)

if __name__=='__main__':main()
